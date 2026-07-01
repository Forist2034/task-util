use std::{
    ffi::{CStr, OsStr},
    io::{Read, Seek, Write},
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
    task::{Status, Task},
    time::TimeData,
};

fn eval_nickel<T: serde::de::DeserializeOwned>(
    file: &str,
    field: Option<&str>,
    args: &[impl AsRef<OsStr>],
    extra_args: &[impl AsRef<OsStr>],
) -> anyhow::Result<T> {
    let mut cmd = std::process::Command::new("nickel");
    cmd.args(["export", "--format", "json"]);
    if let Some(f) = field {
        cmd.args(["--field", f]);
    }
    cmd.arg(file);
    if !extra_args.is_empty() || !args.is_empty() {
        cmd.arg("--");
        cmd.args(args);
        cmd.args(extra_args);
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
    serde_json::from_slice(buf).context("failed to deserialize file")
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

fn read_state_json<R: serde::de::DeserializeOwned>(
    path: &str,
    path_buf: &mut Vec<u8>,
    buf: &mut Vec<u8>,
) -> anyhow::Result<(R, std::fs::File)> {
    path_buf.clear();
    path_buf.extend_from_slice(
        std::path::Path::new(path)
            .file_stem()
            .map_or(path.as_bytes(), |p| p.as_encoded_bytes()),
    );
    path_buf.extend_from_slice(b".json\0");

    let mut file = std::fs::File::from(
        rustix::fs::open(
            std::ffi::CStr::from_bytes_with_nul(&path_buf).unwrap(),
            OFlags::CREATE | OFlags::RDWR | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o666),
        )
        .context("failed to open file")?,
    );
    buf.clear();
    file.read_to_end(buf).context("failed to read file")?;
    let ret = serde_json::from_slice(buf).context("failed to deserialize json")?;
    Ok((ret, file))
}
fn write_state_json(
    file: &mut std::fs::File,
    buf: &mut Vec<u8>,
    val: &impl serde::Serialize,
) -> anyhow::Result<()> {
    buf.clear();
    serde_json::to_writer_pretty(&mut *buf, val).unwrap();
    file.seek(std::io::SeekFrom::Start(0))?;
    file.write_all(&buf)?;
    file.set_len(buf.len() as u64)?;
    Ok(())
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
    pub start_time: DateTime<FixedOffset>,
}

pub struct ProjectHandle {
    pub state: crate::types::project::ProjectDefState,
    file: std::fs::File,
}
impl ProjectHandle {
    pub fn write(&mut self, app: &mut App) -> anyhow::Result<()> {
        write_state_json(&mut self.file, &mut app.val_buf, &self.state)
    }
}

pub struct TagsHandle {
    pub state: crate::types::tag::TagsState,
    file: std::fs::File,
}
impl TagsHandle {
    pub fn write(&mut self, app: &mut App) -> anyhow::Result<()> {
        write_state_json(&mut self.file, &mut app.val_buf, &self.state)
    }
}

const NO_ARGS: &[&str] = &[];

pub struct StopTimeOpt<'a, A> {
    pub task_index: usize,
    pub def_file: Option<&'a str>,
    pub done: bool,
    pub args: &'a [A],
    pub stop_time: Option<DateTime<FixedOffset>>,
}
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
    pub fn read_project(
        &mut self,
        project_path: &str,
    ) -> anyhow::Result<(crate::types::project::ProjectDef, ProjectHandle)> {
        // ignore unset time definition
        let proj: ProjectDef = eval_nickel(project_path, None, NO_ARGS, NO_ARGS)
            .context("failed to eval project definition")?;

        let (state, file) = read_state_json(project_path, &mut self.path_buf, &mut self.val_buf)
            .context("failed to read project state")?;

        Ok((proj, ProjectHandle { state, file }))
    }
    pub fn read_tags(
        &mut self,
        tags_path: &str,
    ) -> anyhow::Result<(Vec<crate::types::tag::TagInfo>, TagsHandle)> {
        let ret = eval_nickel(tags_path, Some("all_tags"), NO_ARGS, NO_ARGS)
            .context("failed to eval tags definition")?;

        let (state, file) = read_state_json(tags_path, &mut self.path_buf, &mut self.val_buf)
            .context("failed to read tags state")?;
        Ok((ret, TagsHandle { state, file }))
    }

    pub fn start_task(&mut self, task: &Task) -> anyhow::Result<Started> {
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
        Ok(Started { start_time })
    }
    pub fn stop_task<'a>(
        &mut self,
        project: &'a ProjectInfo,
        task: &'a mut Task,
        state: &mut ProjectHandle,
        time_opt: StopTimeOpt<impl AsRef<OsStr>>,
    ) -> anyhow::Result<crate::types::time::TimeRecord<&'a ProjectInfo, &'a Task>> {
        let end_time = time_opt
            .stop_time
            .unwrap_or_else(|| chrono::Local::now().fixed_offset());
        println!("task stopped at {end_time} ({})", end_time.to_rfc3339());
        let start: StartedTask = read_json(
            Some(self.time_log_root.as_fd()),
            STARTED_TASK_PATH,
            &mut self.val_buf,
        )
        .context("failed to read started task")?;

        #[derive(Deserialize)]
        struct TimeInfo {
            #[serde(default)]
            data: Option<TimeData>,
            #[serde(default)]
            external_tools: crate::types::time::ExternalTools,
        }
        let time_def: TimeInfo = match time_opt.def_file {
            Some(def) => eval_nickel(
                def,
                None,
                &[
                    &format!("info.task_index={}", time_opt.task_index),
                    &format!("info.done={}", time_opt.done),
                ],
                time_opt.args,
            )
            .context("failed to eval time definition")?,
            None => TimeInfo {
                data: None,
                external_tools: Default::default(),
            },
        };

        if time_opt.done {
            let state = state.state.tasks.entry(task.id).or_default();

            state.status = crate::types::task::Status::Completed;
            state.completed = Some(end_time);

            task.status = crate::types::task::Status::Completed;
            task.completed = Some(end_time);
        }

        let ret = crate::types::time::TimeRecord {
            id: start.id,
            start_time: start.start_time,
            end_time,
            project,
            task: &*task,
            done: time_opt.done,
            data: time_def.data,
            external_tools: time_def.external_tools,
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

        Ok(ret)
    }
}
