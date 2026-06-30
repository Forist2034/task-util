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
        task: usize,
    },
    Start {
        project: String,
        task: usize,
    },
    Stop {
        #[arg(long)]
        stop_time: Option<String>,
        #[arg(long)]
        done: bool,
        #[arg(long)]
        time_data: Option<String>,
        project: String,
        task: usize,
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
    radicale: task_util::ext_tools::radicale::Config,
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
    let mut radicale =
        task_util::ext_tools::radicale::Radicale::new(cfg_root.as_fd(), &cfg.radicale)
            .context("failed to init radicale")?;
    match cli.cmd {
        Cmd::Start { project, task } => {
            let (proj, _state) = native
                .read_project(&project)
                .context("failed to read project")?;
            let task = proj.tasks.get(task).context("invalid task index")?;
            let started = native.start_task(task).context("failed to start task")?;
            taskw
                .start_task(&proj.project, task, started.start_time)
                .context("failed to start task for taskwarrior")?;
            Ok(())
        }
        Cmd::Stop {
            stop_time,
            project,
            task: task_idx,
            time_data,
            done,
            params,
        } => {
            let (mut proj, mut state) = native
                .read_project(&project)
                .context("failed to read project")?;
            let task = proj.tasks.get_mut(task_idx).context("invalid task index")?;
            let rec = native
                .stop_task(
                    &proj.project,
                    task,
                    &mut state,
                    task_util::native::StopTimeOpt {
                        def_file: time_data.as_ref().map(String::as_str),
                        args: &params,
                        done,
                        task_index: task_idx,
                        stop_time: match stop_time {
                            Some(t) => Some(
                                chrono::DateTime::<chrono::FixedOffset>::parse_from_rfc3339(&t)
                                    .context("invalid stop time")?,
                            ),
                            None => None,
                        },
                    },
                )
                .context("failed to stop task")?;
            state
                .write(&mut native)
                .context("failed to write project state")?;
            task_util::ext_tools::timewarrior::import_record(&rec)
                .context("failed to import record to time warrior")?;
            if done {
                taskw
                    .finish_task(rec.project, rec.task)
                    .context("failed to finish taskwarrior task")?;
                radicale
                    .write_task(task_idx, rec.project, rec.task)
                    .context("failed to update radicale task")?;
            } else {
                taskw
                    .stop_task(rec.project, rec.task)
                    .context("failed to stop taskwarrior task")?;
            }
            Ok(())
        }
        Cmd::AddProject { update: _, project } => {
            let (proj, mut state) = native
                .read_project(&project)
                .context("failed to read project")?;
            taskw
                .add_tasks(&proj.project, &proj.tasks)
                .context("failed to add tasks to taskwarrior")?;
            radicale
                .write_project(&proj)
                .context("failed to write project to radicale")?;
            state.write(&mut native).context("failed to save state")
        }
        Cmd::AddTask {
            update: _,
            project,
            task: task_idx,
        } => {
            let (proj, mut state) = native
                .read_project(&project)
                .context("failed to read project task")?;
            let task = proj.tasks.get(task_idx).context("invalid task index")?;
            taskw
                .add_tasks(&proj.project, std::slice::from_ref(task))
                .context("failed to add project task to taskwarrior")?;
            radicale
                .write_task(task_idx, &proj.project, task)
                .context("failed to write task to radicale")?;
            state.write(&mut native).context("failed to save state")
        }
    }
}
