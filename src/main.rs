use anyhow::Result;
use clap::Parser;
use forceops::cli::{Cli, Commands};
use forceops::config::ForceOpsConfig;
use forceops::deleter::FileAndDirectoryDeleter;
use forceops::elevation;
use forceops::lock_checker;
use forceops::utils;
use std::process::ExitCode;
use tracing::error;

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .with_target(false)
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();

    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            error!("{e}");
            ExitCode::from(1)
        }
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Delete {
            files,
            force,
            disable_elevate,
            retry_delay,
            max_retries,
        } => {
            let config = ForceOpsConfig {
                max_retries,
                retry_delay_ms: retry_delay,
                disable_elevate,
            };

            let run_delete = || -> Result<()> {
                let deleter = FileAndDirectoryDeleter::new(config.clone());
                for file in &files {
                    let path = utils::combine_with_cwd_and_get_absolute_path(file);
                    deleter.delete_file_or_directory(&path, force)?;
                }
                Ok(())
            };

            if disable_elevate {
                run_delete()?;
            } else {
                elevation::run_with_relaunch_as_elevated(run_delete, || {
                    let mut args: Vec<String> = std::env::args().collect();
                    if !args.iter().any(|a| a == "-f" || a == "--force") {
                        args.push("-f".to_string());
                    }
                    args
                })?;
            }
        }
        Commands::List { file_or_directory } => {
            let path = utils::combine_with_cwd_and_get_absolute_path(&file_or_directory);
            let processes = lock_checker::get_locks(&path)?;

            println!("ProcessId,ExecutableName,ApplicationName");
            for process in processes {
                println!(
                    "{},{},{}",
                    process.process_id,
                    process.executable_name.as_deref().unwrap_or("<null>"),
                    process.application_name.as_deref().unwrap_or("<null>")
                );
            }
        }
    }

    Ok(())
}
