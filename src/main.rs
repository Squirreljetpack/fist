use std::{fs::OpenOptions, io::Write, path::PathBuf, process::exit};

use cba::{
    _dbg, _ibog,
    bait::{OptionExt, ResultExt},
    bo::{load_type_or_default, write_str},
    bog::{self, BogOkExt},
    ebog,
};
use fist::{
    cli::{
        Cli, SubCmd, ToolsCmd,
        handlers::handle_subcommand,
        paths::{BINARY_FULL, actions_dir, actions_path, lessfilter_cfg_path, pager_cfg_path},
    },
    config::Config,
    errors::CliError,
    menu::MenuActions,
    run::state::MENU_ACTIONS,
};
use matchmaker::MatchError;

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let cli = Cli::parse_custom();
    #[allow(unused)]
    let mut verbosity = cli.opts.verbosity();

    bog::init_bogger(true, true);
    if matches!(
        cli.subcommand,
        SubCmd::Tools(ToolsCmd {
            tool: Some(fist::cli::SubTool::Shell { .. }),
            ..
        })
    ) {
        bog::init_filter(0); // don't break shell init
    }

    // update configs when debug
    #[cfg(debug_assertions)]
    {
        use fist::cli::paths::{config_path, mm_cfg_path};

        // maximum verbosity in debug
        verbosity = 10;

        if cli.opts.mm_config == mm_cfg_path() && cli.opts.config == config_path() {
            write_str(config_path(), include_str!("../assets/config/dev.toml"))._ebog();
            write_str(mm_cfg_path(), include_str!("../assets/config/mm.dev.toml"))._ebog();
            write_str(
                lessfilter_cfg_path(),
                include_str!("../assets/config/lessfilter.dev.toml"),
            )
            ._ebog();
            write_str(
                pager_cfg_path(),
                include_str!("../assets/config/pager.dev.toml"),
            )
            ._ebog();
            write_str(
                actions_path(),
                include_str!("../assets/config/actions.dev.toml"),
            )
            ._ebog();
        }
    }

    // load config
    let mut cfg: Config = load_type_or_default(&cli.opts.config, |s| toml::from_str(s));
    cfg.override_from(&cli.opts);

    // menu actions live in their own file (actions.toml + the actions/
    // folder); a broken file is a hard error naming the file
    let actions = match MenuActions::load_all(actions_path(), actions_dir()) {
        Ok(a) => a,
        Err(e) => {
            ebog!("{e}");
            exit(1);
        }
    };
    MENU_ACTIONS
        .set(actions)
        .expect("MENU_ACTIONS initialized more than once");

    if cli.opts.dump_config {
        dump_config(&cli.opts, &cfg);
    }

    // ensure necessary directories/files (scripts) exist
    check(&cfg);

    let (log_path, append) = if matches!(
        cli.subcommand,
        SubCmd::Tools(ToolsCmd { tool: Some(_), .. }) | SubCmd::Open(_)
    ) {
        (cfg.tools_log_path(), cfg.misc.tools_append_mode_logging)
    } else {
        (cfg.log_path(), cfg.misc.append_mode_logging)
    };
    init_logger(verbosity, log_path, append);

    _dbg!(&cfg);
    match handle_subcommand(cli, cfg).await {
        Ok(()) => (),
        Err(CliError::Handled) => exit(1),
        Err(e) => {
            let code = match e {
                CliError::MatchError(MatchError::EventLoopClosed) => 127,
                CliError::MatchError(MatchError::Abort(i)) => exit(i),
                CliError::MatchError(MatchError::NoMatch) => {
                    if verbosity >= 1 {
                        ebog!("{e}")
                    }
                    exit(22)
                }

                _ => 1,
            };
            ebog!("{e}");
            exit(code);
        }
    }
}

fn init_logger(verbosity: u8, log_path: PathBuf, append: bool) {
    // init bogger
    bog::init_bogger(true, true);
    bog::init_filter(verbosity);

    // init levels from `RUST_LOG`
    let mut builder = env_logger::Builder::from_default_env();

    // override levels
    let rust_log = std::env::var("RUST_LOG").ok().map(|val| val.to_lowercase());
    if rust_log.is_none() {
        #[cfg(debug_assertions)]
        {
            use log::LevelFilter::*;
            builder
                .filter(None, Info)
                .filter(Some("nucleo"), Debug)
                .filter(Some("matchmaker"), Trace)
                .filter(Some(BINARY_FULL), Trace);
        }
        #[cfg(not(debug_assertions))]
        {
            // set style
            builder
                .format_module_path(false)
                .format_target(false)
                .format_timestamp(None);

            let level = cba::bother::level_filter::from_verbosity(verbosity);
            builder
                .filter(Some("sqlx"), level)
                .filter(Some("cba"), level)
                .filter(Some("matchmaker"), level)
                .filter(Some(BINARY_FULL), level);
        }
    }

    // open log file in open/append
    let mut opts = OpenOptions::new();
    opts.create(true);
    if append {
        opts.append(true);
    } else {
        opts.truncate(true).write(true);
    }

    // target log file
    if let Some(log_file) = opts
        .open(log_path)
        .prefix("Failed to open log file")
        ._wbog()
    {
        builder.target(env_logger::Target::Pipe(Box::new(log_file)));
    }

    builder.init();
}

fn dump_config(opts: &fist::cli::CliOpts, cfg: &Config) {
    let lessfilter_cfg_path = lessfilter_cfg_path();
    // if stdout: dump the default cfg (with comments)
    // + (if not yet existing), dump the default run cfg
    if atty::is(atty::Stream::Stdout) {
        macro_rules! init_config {
            ($path:expr, $asset:expr) => {
                let path = $path;
                if !path.exists() && write_str(path, include_str!($asset))._ebog().is_some() {
                    _ibog!("Wrote config to {}", path.to_string_lossy());
                }
            };
        }

        init_config!(&opts.config, "../assets/config/config.toml");
        init_config!(&opts.mm_config, "../assets/config/mm.toml");
        init_config!(&lessfilter_cfg_path, "../assets/config/lessfilter.toml");
        init_config!(&pager_cfg_path(), "../assets/config/pager.toml");
        init_config!(&actions_path(), "../assets/config/actions.toml");
    } else {
        // if piped: dump the current cfg
        let contents = toml::to_string_pretty(&cfg).expect("failed to serialize to TOML");
        std::io::stdout()
            .write_all(contents.as_bytes())
            .ok()
            .or_exit();

        #[cfg(debug_assertions)]
        {
            use fist::run::mm_config::get_mm_cfg;

            std::io::stdout()
                .write_all(b"\n---------------- mm.toml ----------------\n")
                .unwrap();
            let mm_cfg = get_mm_cfg(&opts.mm_config, cfg);
            let contents = toml::to_string_pretty(&mm_cfg).expect("failed to serialize to TOML");
            std::io::stdout().write_all(contents.as_bytes())._ebog();
        }
    }

    exit(0);
}

fn check(cfg: &Config) {
    cfg.check_dirs_or_exit();
}
