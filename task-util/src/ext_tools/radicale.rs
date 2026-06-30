use std::{
    ffi::CStr,
    fmt::Write as _,
    io::Write as _,
    os::fd::{AsFd, BorrowedFd, OwnedFd},
};

use anyhow::Context;
use rustix::{
    fs::{Mode, OFlags},
    io::Errno,
};

use crate::ext_tools::ical::ICalTodoBuilder;

#[derive(serde::Serialize)]
struct ProjectProp<'a> {
    #[serde(rename = "C:supported-calendar-component-set")]
    component_set: &'static str,
    #[serde(rename = "D:displayname")]
    name: &'a str,
    #[serde(rename = "C:calendar-description")]
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<&'a String>,
    #[serde(rename = "ICAL:calendar-color")]
    color: &'a str,
    tag: &'static str,
    #[serde(rename = "{http://owncloud.org/ns}calendar-enabled")]
    calender_enabled: &'static str,
}

#[derive(serde::Deserialize)]
pub struct Config {
    path: String,
}
pub struct Radicale {
    root: OwnedFd,
    path_buf: String,
    ical_buf: ICalTodoBuilder,
}
impl Radicale {
    pub fn new(root: BorrowedFd<'_>, cfg: &Config) -> anyhow::Result<Self> {
        Ok(Self {
            root: rustix::fs::openat(
                root,
                &cfg.path,
                OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .context("failed to open root dir")?,
            path_buf: String::new(),
            ical_buf: ICalTodoBuilder::new(),
        })
    }
    pub fn write_task(
        &mut self,
        idx: usize,
        project: &crate::types::project::ProjectInfo,
        task: &crate::types::task::Task,
    ) -> anyhow::Result<()> {
        self.ical_buf.clear();
        self.ical_buf.from_task(idx, task);
        let val = self.ical_buf.finish();

        self.path_buf.clear();
        let _ = write!(&mut self.path_buf, "{}/{}.ics\0", project.id, task.id);
        std::fs::File::from(
            rustix::fs::openat(
                self.root.as_fd(),
                CStr::from_bytes_with_nul(self.path_buf.as_bytes()).unwrap(),
                OFlags::CREATE | OFlags::TRUNC | OFlags::WRONLY | OFlags::CLOEXEC,
                Mode::from_raw_mode(0o666),
            )
            .context("failed to open ics file")?,
        )
        .write_all(val.as_bytes())
        .context("failed to write ics file")
    }
    pub fn write_project(
        &mut self,
        project: &crate::types::project::ProjectDef,
    ) -> anyhow::Result<()> {
        let d = serde_json::to_vec(&ProjectProp {
            component_set: "VTODO",
            name: &project.project.name,
            description: project.project.description.as_ref(),
            color: &project.project.color,
            tag: "VCALENDAR",
            calender_enabled: "1",
        })
        .unwrap();

        self.path_buf.clear();
        let _ = write!(&mut self.path_buf, "{}\0", project.project.id);
        match rustix::fs::mkdirat(
            self.root.as_fd(),
            std::ffi::CStr::from_bytes_with_nul(self.path_buf.as_bytes()).unwrap(),
            Mode::from_raw_mode(0o755),
        ) {
            Ok(()) => (),
            Err(Errno::EXIST) => (),
            Err(e) => return Err(anyhow::Error::new(e).context("failed to create project dir")),
        }

        self.path_buf.pop();
        self.path_buf.push_str("/.Radicale.props\0");
        std::fs::File::from(
            rustix::fs::openat(
                self.root.as_fd(),
                std::ffi::CStr::from_bytes_with_nul(self.path_buf.as_bytes()).unwrap(),
                OFlags::CREATE | OFlags::TRUNC | OFlags::WRONLY | OFlags::CLOEXEC,
                Mode::from_raw_mode(0o666),
            )
            .context("failed to open project prop file")?,
        )
        .write_all(&d)
        .context("failed to write project props")?;

        for (idx, t) in project.tasks.iter().enumerate() {
            self.write_task(idx, &project.project, t)?;
        }
        Ok(())
    }
}
