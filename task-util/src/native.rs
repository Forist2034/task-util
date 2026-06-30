use std::{
    ffi::{CStr, OsStr},
    io::{Read, Write},
    os::fd::{AsFd, BorrowedFd, OwnedFd},
    path::Path,
    time::SystemTime,
};

use anyhow::Context as _;
use chrono::{DateTime, Datelike, FixedOffset};
use rustix::fs::{AtFlags, Mode, OFlags};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::{
    project::{ProjectDef, ProjectInfo},
    task::{Status, Task, TaskRef},
};

fn eval_nickel<T: serde::de::DeserializeOwned>(
    file: &str,
    field: Option<&str>,
    args: &[impl AsRef<OsStr>],
) -> anyhow::Result<T> {
    let mut cmd = std::process::Command::new("nickel");
    cmd.args(["export", "--format", "json"]);
    if let Some(f) = field {
        cmd.args(["--field", f]);
    }
    cmd.arg(file);
    if !args.is_empty() {
        cmd.arg("--");
        cmd.args(args);
    }
    let out = cmd
        .stdin(std::process::Stdio::null())
        .output()
        .context("failed to get task info")?;
    if !out.status.success() {
        anyhow::bail!(
            "nickel export returns: {:?}:\n{}",
            out.status,
            out.stderr.escape_ascii()
        )
    }
    serde_json::from_slice(&out.stdout).context("failed to deserialize output")
}
fn read_json<T: serde::de::DeserializeOwned>(
    root: Option<BorrowedFd>,
    path: impl rustix::path::Arg,
    buf: &mut Vec<u8>,
) -> anyhow::Result<T> {
    buf.clear();
    std::fs::File::from(
        rustix::fs::openat(
            root.unwrap_or(rustix::fs::CWD),
            path,
            OFlags::RDONLY | OFlags::CLOEXEC,
            Mode::all(),
        )
        .context("failed to open file")?,
    )
    .read_to_end(buf)
    .context("failed to read file")?;
    serde_json::from_slice(&buf).context("failed to deserialize file")
}
fn create_dir_all(root: BorrowedFd<'_>, path: &Path) -> Result<(), rustix::io::Errno> {
    match rustix::fs::mkdirat(root, path, Mode::from_raw_mode(0o750)) {
        Ok(()) => Ok(()),
        e @ Err(rustix::io::Errno::NOENT) => match path.parent() {
            Some(p) => {
                create_dir_all(root, p)?;
                rustix::fs::mkdirat(root, path, Mode::from_raw_mode(0o750))
            }
            None => e,
        },
        e => e,
    }
}
struct WriteOptions {
    overwrite: bool,
    create_dir: bool,
    mode: Mode,
}
impl WriteOptions {
    const DEFAULT: Self = Self {
        overwrite: false,
        create_dir: false,
        mode: Mode::from_raw_mode(0o440),
    };
}
fn write_json(
    root: Option<BorrowedFd>,
    path: impl rustix::path::Arg + Copy,
    opts: WriteOptions,
    buf: &mut Vec<u8>,
    val: &impl serde::Serialize,
) -> anyhow::Result<()> {
    buf.clear();
    serde_json::to_writer_pretty(&mut *buf, val).unwrap();
    let dirfd = root.unwrap_or(rustix::fs::CWD);
    let flags = if opts.overwrite {
        OFlags::CREATE | OFlags::TRUNC | OFlags::WRONLY | OFlags::CLOEXEC
    } else {
        OFlags::CREATE | OFlags::EXCL | OFlags::WRONLY | OFlags::CLOEXEC
    };
    let mut file = match rustix::fs::openat(dirfd, path, flags, opts.mode) {
        Ok(r) => std::fs::File::from(r),
        Err(e @ rustix::io::Errno::NOENT) if opts.create_dir => {
            match std::path::Path::new(path.as_str().unwrap()).parent() {
                Some(p) => {
                    create_dir_all(dirfd, p).context("failed to create parent dir")?;
                    rustix::fs::openat(dirfd, path, flags, opts.mode)
                        .context("failed to create output file")?
                        .into()
                }
                None => return Err(anyhow::Error::new(e).context("failed to create output file")),
            }
        }
        Err(e) => return Err(anyhow::Error::new(e).context("failed to create output file")),
    };
    file.write_all(buf).context("failed to write output file")
}

#[derive(Serialize, Deserialize)]
struct StartedTask {
    id: Uuid,
    start_time: DateTime<FixedOffset>,
}

const STARTED_TASK_PATH: &CStr = c".started_task";

#[derive(Deserialize)]
pub struct Config {
    time_log_path: String,
}

#[derive(Debug)]
pub struct Started {
    pub project: ProjectInfo,
    pub index: usize,
    pub task: Task,
    pub start_time: DateTime<FixedOffset>,
}

const NO_ARGS: &[&str] = &[];
pub struct App {
    time_log_root: OwnedFd,
    path_buf: Vec<u8>,
    val_buf: Vec<u8>,
}
impl App {
    pub fn new(root: BorrowedFd<'_>, cfg: &Config) -> anyhow::Result<Self> {
        Ok(Self {
            time_log_root: rustix::fs::openat(
                root,
                &cfg.time_log_path,
                OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
                Mode::all(),
            )
            .context("failed to open time log dir")?,
            path_buf: Vec::new(),
            val_buf: Vec::new(),
        })
    }
    pub fn read_project<T: serde::de::DeserializeOwned>(
        &mut self,
        project_path: &str,
    ) -> anyhow::Result<crate::types::project::ProjectDef<T>> {
        eval_nickel(project_path, None, NO_ARGS)
    }
    pub fn read_project_task(
        &mut self,
        project_path: &str,
        task_path: &str,
    ) -> anyhow::Result<(ProjectInfo, usize, crate::types::task::Task)> {
        let proj: ProjectDef<TaskRef> = self
            .read_project(project_path)
            .context("failed to eval project definition")?;
        let task: crate::types::task::Task = eval_nickel(task_path, Some("task"), NO_ARGS)
            .context("failed to eval task definition")?;
        let idx = proj
            .tasks
            .iter()
            .enumerate()
            .find(|(_, v)| v.id == task.id)
            .context("task is not in project")?
            .0;
        Ok((proj.project, idx, task))
    }

    pub fn start_task(&mut self, project: &str, task: &str) -> anyhow::Result<Started> {
        let (project, idx, task) = self.read_project_task(project, task)?;
        if task.status != Status::Pending {
            anyhow::bail!("only pending task can be started");
        }
        let ts = SystemTime::now();
        let start_time = chrono::DateTime::<chrono::Local>::from(ts).fixed_offset();
        let id = uuid::Uuid::new_v7({
            let dur = ts.duration_since(SystemTime::UNIX_EPOCH).unwrap();
            uuid::Timestamp::from_unix(uuid::NoContext, dur.as_secs(), dur.subsec_nanos())
        });
        write_json(
            Some(self.time_log_root.as_fd()),
            STARTED_TASK_PATH,
            WriteOptions::DEFAULT,
            &mut self.val_buf,
            &StartedTask { id, start_time },
        )
        .context("failed to write started task info")?;

        println!(
            "task {id} started at {start_time} ({})",
            start_time.to_rfc3339()
        );
        Ok(Started {
            project,
            index: idx,
            task,
            start_time,
        })
    }
    pub fn stop_task(
        &mut self,
        project: &str,
        task: &str,
        done: bool,
        stop_time: Option<DateTime<FixedOffset>>,
        args: &[impl AsRef<OsStr>],
    ) -> anyhow::Result<(usize, crate::types::time::TimeRecord)> {
        let end_time = stop_time.unwrap_or_else(|| chrono::Local::now().fixed_offset());
        println!("task stopped at {end_time} ({})", end_time.to_rfc3339());
        let start: StartedTask = read_json(
            Some(self.time_log_root.as_fd()),
            STARTED_TASK_PATH,
            &mut self.val_buf,
        )
        .context("failed to read started task")?;

        #[derive(Deserialize)]
        struct TimeDef {
            task: crate::types::task::Task,
            time: crate::types::time::TimeInfo,
        }
        let project: crate::types::project::ProjectDef<TaskRef> = self
            .read_project(project)
            .context("failed to eval project definition")?;
        let mut time_def: TimeDef =
            eval_nickel(task, None, args).context("failed to eval time definition")?;
        let idx = project
            .tasks
            .iter()
            .enumerate()
            .find(|(_, t)| t.id == time_def.task.id)
            .context("can't find task in project")?
            .0;

        if done {
            time_def.task.status = crate::types::task::Status::Completed;
            time_def.task.completed = Some(end_time);

            self.path_buf.clear();
            self.path_buf.extend_from_slice(
                std::path::Path::new(task)
                    .file_stem()
                    .map_or(task.as_bytes(), |p| p.as_encoded_bytes()),
            );
            self.path_buf.extend_from_slice(b".json\0");
            write_json(
                None,
                std::ffi::CStr::from_bytes_with_nul(&self.path_buf).unwrap(),
                WriteOptions {
                    overwrite: true,
                    create_dir: false,
                    mode: Mode::RUSR | Mode::WUSR | Mode::RGRP | Mode::WGRP,
                },
                &mut self.val_buf,
                &crate::types::task::TaskState {
                    status: crate::types::task::Status::Completed,
                    completed: Some(end_time),
                },
            )
            .context("failed to write completed json")?;
        }

        let ret = crate::types::time::TimeRecord {
            id: start.id,
            start_time: start.start_time,
            end_time,
            project: project.project,
            task: time_def.task,
            done,
            data: time_def.time.data,
            external_tools: time_def.time.external_tools,
        };

        let _ = {
            self.path_buf.clear();
            let year = ret.start_time.year_ce().1;
            write!(
                &mut self.path_buf,
                "{year}/{year}-{:02}/{:02}/{}.json\0",
                ret.start_time.month(),
                ret.start_time.day(),
                ret.id
            )
        };
        write_json(
            Some(self.time_log_root.as_fd()),
            std::ffi::CStr::from_bytes_until_nul(&self.path_buf).unwrap(),
            WriteOptions {
                create_dir: true,
                ..WriteOptions::DEFAULT
            },
            &mut self.val_buf,
            &ret,
        )
        .context("failed to write time record")?;

        rustix::fs::unlinkat(
            self.time_log_root.as_fd(),
            STARTED_TASK_PATH,
            AtFlags::empty(),
        )
        .context("failed to remove started task file")?;

        Ok((idx, ret))
    }
}
