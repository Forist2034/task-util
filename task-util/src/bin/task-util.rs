use std::os::fd::AsFd;

use anyhow::Context;
use clap::Parser;
use rustix::fs::{Mode, OFlags};

#[derive(clap::Subcommand)]
enum Cmd {
    Start {
        task: String,
    },
    Stop {
        #[arg(long)]
        stop_time: Option<String>,
        #[arg(long)]
        done: bool,
        task: String,
        #[arg(last = true)]
        params: Vec<String>,
    },
}
#[derive(clap::Parser)]
struct Cli {
    #[arg(long)]
    config: String,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(serde::Deserialize)]
struct Config {
    native: task_util::native::Config,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let cfg_root = rustix::fs::open(
        std::path::Path::new(&cli.config)
            .parent()
            .unwrap_or(std::path::Path::new(".")),
        OFlags::PATH | OFlags::CLOEXEC,
        Mode::all(),
    )
    .context("failed to open config root")?;
    let cfg: Config =
        serde_json::from_slice(&std::fs::read(&cli.config).context("failed to read config file")?)
            .context("failed to parse config file")?;
    let mut native = task_util::native::App::new(cfg_root.as_fd(), &cfg.native)
        .context("failed to init native app")?;
    match cli.cmd {
        Cmd::Start { task } => {
            let (_task, _start) = native.start_task(&task).context("failed to start task")?;
            Ok(())
        }
        Cmd::Stop {
            stop_time,
            task,
            done,
            params,
        } => {
            let _rec = native
                .stop_task(
                    &task,
                    done,
                    match stop_time {
                        Some(t) => Some(
                            chrono::DateTime::<chrono::FixedOffset>::parse_from_rfc3339(&t)
                                .context("invalid stop time")?,
                        ),
                        None => None,
                    },
                    &params,
                )
                .context("failed to stop task")?;
            Ok(())
        }
    }
}
