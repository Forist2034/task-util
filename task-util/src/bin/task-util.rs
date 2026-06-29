use std::os::fd::AsFd;

use anyhow::Context;
use clap::Parser;
use rustix::fs::{Mode, OFlags};

#[derive(clap::Subcommand)]
enum Cmd {
    AddProject {
        #[arg(long)]
        update: bool,
        project: String,
    },
    AddTask {
        #[arg(long)]
        update: bool,
        project: String,
        task: String,
    },
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
    let mut taskw = task_util::ext_tools::taskwarrior::Taskwarrior::new();
    match cli.cmd {
        Cmd::Start { task } => {
            let (task, start) = native.start_task(&task).context("failed to start task")?;
            taskw
                .start_task(&task, start)
                .context("failed to start task for taskwarrior")?;
            Ok(())
        }
        Cmd::Stop {
            stop_time,
            task,
            done,
            params,
        } => {
            let rec = native
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
            task_util::ext_tools::timewarrior::import_record(&rec)
                .context("failed to import record to time warrior")?;
            if done {
                taskw.finish_task(&rec.task)
            } else {
                taskw.stop_task(&rec.task)
            }
            .context("failed to stop taskwarrior task")?;
            Ok(())
        }
        Cmd::AddProject { update: _, project } => {
            let proj = native
                .read_project(&project)
                .context("failed to read project")?;
            taskw
                .add_tasks(&proj.tasks)
                .context("failed to add tasks to taskwarrior")
        }
        Cmd::AddTask {
            update: _,
            project,
            task,
        } => {
            let (_idx, task) = native
                .read_project_task(&project, &task)
                .context("failed to read project task")?;
            taskw
                .add_tasks(std::slice::from_ref(&task))
                .context("failed to add project task to taskwarrior")
        }
    }
}
