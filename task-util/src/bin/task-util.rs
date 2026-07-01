use std::os::fd::AsFd;

use anyhow::Context;
use clap::Parser;
use rustix::fs::{Mode, OFlags};

#[derive(clap::Subcommand)]
enum Cmd {
    InitProject {
        #[arg(long)]
        root: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        color: String,
        output: String,
    },
    AddProject {
        #[arg(long)]
        update: bool,
        project: String,
    },
    InitTags {
        output: String,
    },
    NewTag {
        #[arg(long)]
        color: String,
        name: String,
    },
    AddTags {
        #[arg(long)]
        update: bool,
        tags: String,
    },
    AddTasks {
        #[arg(long)]
        update: bool,
        project: String,
    },
    ListTasks {
        project: String,
    },
    NewTask {
        name: String,
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
    let rt = tokio::runtime::Runtime::new().context("failed to create tokio runtime")?;
    let mut native = task_util::native::App::new(cfg_root.as_fd(), &cfg.native)
        .context("failed to init native app")?;
    let mut taskw = task_util::ext_tools::taskwarrior::Taskwarrior::new();
    let mut radicale =
        task_util::ext_tools::radicale::Radicale::new(cfg_root.as_fd(), &cfg.radicale)
            .context("failed to init radicale")?;
    let mut super_productivity =
        task_util::ext_tools::super_productivity::SuperProductivity::new(rt.handle().clone())
            .context("failed to init super-productivity")?;
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
            super_productivity
                .start_task(&task, started.start_time)
                .context("failed to start task for super-productivity")?;
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
                super_productivity
                    .finish_task(rec.task, rec.end_time)
                    .context("failed to finish super-productivity task")?;
            } else {
                taskw
                    .stop_task(rec.project, rec.task)
                    .context("failed to stop taskwarrior task")?;
                super_productivity
                    .stop_task(rec.task)
                    .context("failed to stop super-productivity task")?;
            }
            Ok(())
        }
        Cmd::InitProject {
            root,
            name,
            color,
            output,
        } => {
            let mut path = std::path::PathBuf::from(&output);

            path.set_extension("ncl");
            std::fs::write(
                &path,
                format!(
                    include_str!("./templates/project.ncl"),
                    file = &output,
                    id = uuid::Uuid::new_v4(),
                    root = root.escape_debug(),
                    name = name.escape_debug(),
                    created = chrono::Local::now().fixed_offset().to_rfc3339(),
                    color = &color
                ),
            )
            .context("failed to write project def file")?;

            path.set_extension("json");
            std::fs::write(&path, r#"{"project":{},"tasks":{}}"#)
                .context("failed to write json file")
        }
        Cmd::AddProject { update, project } => {
            let (proj, mut state) = native
                .read_project(&project)
                .context("failed to read project")?;
            radicale
                .write_project(&proj)
                .context("failed to write project to radicale")?;
            super_productivity
                .add_project(update, &proj.project, &mut state)
                .context("failed to add project to super-productivity")?;
            state.write(&mut native).context("failed to save state")
        }
        Cmd::InitTags { output } => {
            let mut path = std::path::PathBuf::from(&output);

            path.set_extension("ncl");
            std::fs::write(
                &path,
                format!(include_str!("./templates/tags.ncl"), file = &output),
            )
            .context("failed to write tags def file")?;

            path.set_extension("json");
            std::fs::write(&path, "{}").context("failed to write tags state file")
        }
        Cmd::AddTags { update, tags } => {
            let (tags, mut state) = native.read_tags(&tags).context("failed to read tags")?;
            for t in tags.iter() {
                super_productivity
                    .add_tag(update, t, &mut state)
                    .with_context(|| format!("failed to add tag {} to super-productivity", t.id))?;
            }
            state
                .write(&mut native)
                .context("failed to save tags state")
        }
        Cmd::NewTag { color, name } => {
            println!(
                include_str!("./templates/tag.ncl"),
                id = uuid::Uuid::new_v4(),
                name = name.escape_debug(),
                created = chrono::Local::now().fixed_offset().to_rfc3339(),
                color = &color
            );
            Ok(())
        }
        Cmd::NewTask { name } => {
            let ts = std::time::SystemTime::now();
            let dur = ts
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap();
            println!(
                include_str!("./templates/task.ncl"),
                id = uuid::Uuid::new_v7(uuid::Timestamp::from_unix(
                    uuid::NoContext,
                    dur.as_secs(),
                    dur.subsec_nanos()
                )),
                name = name.escape_debug(),
                created = chrono::DateTime::<chrono::Local>::from(ts)
                    .fixed_offset()
                    .to_rfc3339()
            );
            Ok(())
        }
        Cmd::ListTasks { project } => {
            let (proj, _) = native
                .read_project(&project)
                .context("failed to read project")?;
            for (idx, t) in proj.tasks.iter().enumerate() {
                println!(
                    "{idx:>4} {} {}",
                    match t.status {
                        task_util::types::task::Status::Pending => " ",
                        task_util::types::task::Status::Completed => "x",
                    },
                    t.name
                );
            }
            Ok(())
        }
        Cmd::AddTasks { update, project } => {
            let (proj, mut state) = native
                .read_project(&project)
                .context("failed to read project task")?;
            taskw
                .add_tasks(&proj.project, &proj.tasks)
                .context("failed to add project task to taskwarrior")?;
            for (idx, task) in proj.tasks.iter().enumerate() {
                radicale
                    .write_task(idx, &proj.project, task)
                    .context("failed to write task to radicale")?;
                super_productivity
                    .add_task(update, &proj.project, task, &mut state)
                    .with_context(|| format!("failed to add task {idx} to super-productivity"))?;
            }
            state.write(&mut native).context("failed to save state")
        }
    }
}
