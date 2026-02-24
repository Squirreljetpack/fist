use std::{fs::OpenOptions, io::Write, path::PathBuf, process::exit};

use cli_boilerplate_automation::{
    _ibog,
    bait::ResultExt,
    bo::{load_type_or_default, write_str},
    bog::{self, BogOkExt},
    ebog,
};
use fist::{
    cli::{
        Cli, SubCmd, ToolsCmd,
        handlers::handle_subcommand,
        paths::{BINARY_FULL, config_path, lessfilter_cfg_path},
    },
    config::Config,
    errors::CliError,
};
use matchmaker::MatchError;

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let cli = Cli::parse_custom();
    let verbosity = cli.opts.verbosity();

    bog::init_bogger(true, false);
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
    use fist::cli::paths::mm_cfg_path;
    #[cfg(debug_assertions)]
    if cli.opts.mm_config == mm_cfg_path() && cli.opts.config == config_path() {
        write_str(config_path(), include_str!("../assets/config/dev.toml"))._ebog();
        write_str(mm_cfg_path(), include_str!("../assets/config/mm.dev.toml"))._ebog();
        write_str(
            lessfilter_cfg_path(),
            include_str!("../assets/config/lessfilter.dev.toml"),
        )
        ._ebog();
    }

    // load config

    let mut cfg: Config = load_type_or_default(config_path(), |s| toml::from_str(s));
    cfg.override_from(&cli.opts);

    if cli.opts.dump_config {
        dump_config(&cli.opts, &cfg);
    }

    // ensure necessary directories/files (scripts) exist
    check(&cfg);

    // skip tool logging when not in append mode (mainly lessfilter)
    if !cfg.misc.append_mode_logging
        && (std::env::var("MM_IN_APP").as_deref() == Ok("true")
            || matches!(
                cli.subcommand,
                SubCmd::Tools(ToolsCmd { tool: Some(_), .. }) | SubCmd::Open(_)
            ))
    {
        // skip
    } else {
        init_logger(verbosity, cfg.log_path(), cfg.misc.append_mode_logging);
    }

    match handle_subcommand(cli, cfg).await {
        Ok(()) => (),
        Err(CliError::Handled) => exit(1),
        Err(e) => {
            let code = match e {
                CliError::MatchError(MatchError::EventLoopClosed) => 127,
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

fn init_logger(
    verbosity: u8,
    log_path: PathBuf,
    append: bool,
) {
    // init bogger
    use log::LevelFilter::*;
    bog::init_bogger(true, true);
    bog::init_filter(verbosity);

    // init levels from `RUST_LOG`
    let mut builder = env_logger::Builder::from_default_env();

    // override levels
    let rust_log = std::env::var("RUST_LOG").ok().map(|val| val.to_lowercase());
    if rust_log.is_none() {
        #[cfg(debug_assertions)]
        {
            builder
                .filter(None, Info)
                .filter(Some("nucleo"), Debug)
                .filter(Some("matchmaker"), Debug)
                .filter(Some(BINARY_FULL), Trace);
        }
        #[cfg(not(debug_assertions))]
        {
            use cli_boilerplate_automation::bait::TransformExt;

            // set style
            builder
                .format_module_path(false)
                .format_target(false)
                .format_timestamp(None);

            let level = cli_boilerplate_automation::bother::level_filter::from_verbosity(
                verbosity.transform_if(verbosity > 4, |v| v - 1),
            );
            builder
                .filter(Some("sqlx"), level)
                .filter(Some("cli_boilerplate_automation"), level)
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

fn dump_config(
    opts: &fist::cli::CliOpts,
    cfg: &Config,
) {
    let lessfilter_cfg_path = lessfilter_cfg_path();
    // if stdout: dump the default cfg (with comments)
    // + (if not yet existing), dump the default run cfg
    if atty::is(atty::Stream::Stdout) {
        // todo: prompt about overwriting
        if write_str(&opts.config, include_str!("../assets/config/config.toml"))
            ._ebog()
            .is_some()
        {
            _ibog!("Wrote config to {}", &opts.config.to_string_lossy());
            // overwrite helper files
            Config::default().check_scripts(true);
        } else {
            cfg.check_scripts(true);
        }

        if !opts.mm_config.exists()
            && write_str(&opts.mm_config, include_str!("../assets/config/mm.toml"))
                ._ebog()
                .is_some()
        {
            _ibog!("Wrote config to {}", opts.mm_config.to_string_lossy())
        }
        if !lessfilter_cfg_path.exists()
            && write_str(
                lessfilter_cfg_path,
                include_str!("../assets/config/lessfilter.toml"),
            )
            ._ebog()
            .is_some()
        {
            _ibog!("Wrote config to {}", lessfilter_cfg_path.to_string_lossy())
        }
    } else {
        // if piped: dump the current cfg
        let contents = toml::to_string_pretty(&cfg).expect("failed to serialize to TOML");
        std::io::stdout().write_all(contents.as_bytes())._ebog();

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
    #[cfg(debug_assertions)]
    cfg.check_scripts(true);
    #[cfg(not(debug_assertions))]
    cfg.check_scripts(false);
}
