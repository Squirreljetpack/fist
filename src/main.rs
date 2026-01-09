use std::{fs::OpenOptions, io::Write, path::PathBuf, process};

use cli_boilerplate_automation::{
    bait::{OptionExt, ResultExt},
    bo::{load_type, write_str},
    bog::{self, BogOkExt},
    ebog, ibog,
};
use fist::{
    cli::{
        Cli, SubCmd, ToolsCmd,
        handlers::handle_subcommand,
        paths::{config_path, lessfilter_cfg_path, mm_cfg_path},
    },
    config::Config,
    errors::CliError,
};

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let cli = Cli::parse_custom();

    bog::init_bogger(true, false); // early init bogging

    #[cfg(debug_assertions)]
    if cli.opts.mm_config.is_none() || cli.opts.config.is_none() {
        write_str(config_path(), include_str!("../assets/config/dev.toml"))._ebog();
        write_str(mm_cfg_path(), include_str!("../assets/config/mm.dev.toml"))._ebog();
        write_str(
            lessfilter_cfg_path(),
            include_str!("../assets/config/lessfilter.dev.toml"),
        )
        ._ebog();
    }

    let cfg: Config = if let Some(p) = cli.opts.config.as_deref() {
        load_type(p, |s| toml::from_str(s)).or_exit()
    } else {
        let p = config_path();
        if p.is_file() {
            load_type(p, |s| toml::from_str(s)).or_exit()
        } else {
            toml::from_str(include_str!("../assets/config/config.toml")).__ebog()
        }
    };

    if cli.opts.dump_config {
        let mm_cfg_path = cli.opts.mm_config.as_deref().unwrap_or(mm_cfg_path());
        let lessfilter_cfg_path = lessfilter_cfg_path();
        // if stdout: dump the default cfg (with comments)
        // + (if not yet existing), dump the default run cfg
        if atty::is(atty::Stream::Stdout) {
            let cfg_path = cli.opts.config.as_deref().unwrap_or(config_path());
            // todo: prompt about overwriting
            if write_str(cfg_path, include_str!("../assets/config/config.toml"))
                ._ebog()
                .is_some()
            {
                ibog!("Wrote config to {}", cfg_path.to_string_lossy());
                // overwrite helper files
                Config::default().check_scripts(true);
            } else {
                cfg.check_scripts(true);
            }

            if !mm_cfg_path.exists()
                && write_str(mm_cfg_path, include_str!("../assets/config/mm.toml"))
                    ._ebog()
                    .is_some()
            {
                ibog!("Wrote config to {}", mm_cfg_path.to_string_lossy())
            }
            if !lessfilter_cfg_path.exists()
                && write_str(
                    lessfilter_cfg_path,
                    include_str!("../assets/config/lessfilter.toml"),
                )
                ._ebog()
                .is_some()
            {
                ibog!("Wrote config to {}", lessfilter_cfg_path.to_string_lossy())
            }
        } else {
            // if piped: dump the current cfg
            let contents = toml::to_string_pretty(&cfg).expect("failed to serialize to TOML");
            std::io::stdout().write_all(contents.as_bytes())._ebog();

            #[cfg(debug_assertions)]
            {
                std::io::stdout()
                    .write_all(b"\n---------------- mm.toml ----------------\n")
                    .unwrap();
                let mm_cfg = fist::run::mm_config::get_mm_cfg(mm_cfg_path, &cfg);
                let contents =
                    toml::to_string_pretty(&mm_cfg).expect("failed to serialize to TOML");
                std::io::stdout().write_all(contents.as_bytes())._ebog();
            }
        }

        std::process::exit(0);
    }
    // ensure necessary directories/files (scripts) exist
    cfg.check_dirs_or_exit();
    #[cfg(debug_assertions)]
    cfg.check_scripts(true);
    #[cfg(not(debug_assertions))]
    cfg.check_scripts(false);

    // if atty is not stdin, (this may be a bit unexpected but shouldn't be a big problem)
    if !cfg.misc.append_mode_logging
        && (std::env::var("MM_IN_APP").as_deref() == Ok("true")
            || matches!(
                cli.subcommand,
                SubCmd::Tools(ToolsCmd { tool: Some(_), .. }) | SubCmd::Open(_)
            ))
    {
        // skip tool logging when not in append mode (mainly lessfilter)
    } else {
        init_logger(
            cli.opts.verbosity(),
            cfg.log_path(),
            cfg.misc.append_mode_logging,
        );
    }

    match handle_subcommand(cli, cfg).await {
        Ok(()) => (),
        Err(CliError::Handled) => process::exit(1),
        Err(e) => ebog!("{e}"),
    }
}

fn init_logger(
    verbosity: u8,
    log_path: PathBuf,
    append: bool,
) {
    bog::init_bogger(true, true);
    bog::init_filter(verbosity);

    let rust_log = std::env::var("RUST_LOG").ok().map(|val| val.to_lowercase());

    let mut builder = env_logger::Builder::from_default_env();
    use fist::cli::BINARY_FULL;

    if rust_log.is_none() {
        #[cfg(debug_assertions)]
        {
            builder
                .filter(None, log::LevelFilter::Info)
                .filter(Some("matchmaker"), log::LevelFilter::Debug)
                .filter(Some(BINARY_FULL), log::LevelFilter::Trace);
        }
        #[cfg(not(debug_assertions))]
        {
            builder
                .format_module_path(false)
                .format_target(false)
                .format_timestamp(None);

            let level = cli_boilerplate_automation::bother::level_filter_from_env();

            builder
                .filter(Some("matchmaker"), level)
                .filter(Some(BINARY_FULL), level);
        }
    }

    let mut opts = OpenOptions::new();

    opts.create(true);
    if append {
        opts.append(true);
    } else {
        opts.truncate(true).write(true);
    }

    if let Some(log_file) = opts
        .open(log_path)
        .prefix("Failed to open log file")
        ._wbog()
    {
        builder.target(env_logger::Target::Pipe(Box::new(log_file)));
    }

    builder.init();
}
