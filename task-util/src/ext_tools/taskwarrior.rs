use std::io::Write;

use anyhow::Context;
use chrono::{Datelike as _, Timelike as _};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Status {
    Pending,
    Completed,
}

#[derive(Debug)]
pub(super) struct DateTime(pub(super) chrono::DateTime<chrono::Utc>);
impl serde::Serialize for DateTime {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        const LEN: usize = "YYYYMMDDTHHMMSSZ".len();
        let mut buf = std::io::Cursor::new([0; LEN]);
        let _ = write!(
            &mut buf,
            "{year:04}{month:02}{day:02}T{h:02}{m:02}{s:02}Z",
            year = self.0.year_ce().1,
            month = self.0.month(),
            day = self.0.day(),
            h = self.0.hour(),
            m = self.0.minute(),
            s = self.0.second()
        );
        std::str::from_utf8(&buf.into_inner())
            .unwrap()
            .serialize(serializer)
    }
}
impl<'de> serde::Deserialize<'de> for DateTime {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        serde::Deserialize::deserialize(deserializer).map(Self)
    }
}

#[derive(Debug, Serialize)]
struct Task<S> {
    id: usize,
    uuid: Uuid,
    description: S,
    status: Status,
    entry: DateTime,
    #[serde(skip_serializing_if = "Option::is_none")]
    start: Option<DateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    end: Option<DateTime>,
    project: String,
    tags: Vec<S>,
}

fn to_task<'a>(
    id: usize,
    p: &'a crate::types::project::ProjectInfo,
    t: &'a crate::types::task::Task,
    start: Option<chrono::DateTime<chrono::FixedOffset>>,
) -> Task<&'a str> {
    let status = match t.status {
        crate::types::task::Status::Pending => Status::Pending,
        crate::types::task::Status::Completed => Status::Completed,
    };
    Task {
        id,
        uuid: t.id,
        description: &t.name,
        status,
        entry: DateTime(t.created.to_utc()),
        start: start.map(|v| DateTime(v.to_utc())),
        end: t.completed.map(|v| DateTime(v.to_utc())),
        project: format!("{}.{}", p.root, p.name),
        tags: t.tags.iter().map(|t| t.name.as_str()).collect(),
    }
}

#[derive(Debug)]
pub struct Taskwarrior {}
impl Taskwarrior {
    pub fn new() -> Self {
        Self {}
    }

    fn import_task(&mut self, tasks: &[Task<&str>]) -> anyhow::Result<()> {
        let data = serde_json::to_vec(tasks).unwrap();
        let mut child = std::process::Command::new("task")
            .arg("import")
            .stdin(std::process::Stdio::piped())
            .spawn()
            .context("failed to spawn taskwarrior")?;
        child.stdin.take().unwrap().write_all(&data)?;
        let status = child.wait().context("failed to wait taskwarrior")?;
        if !status.success() {
            anyhow::bail!("taskwarrior returned: {status:?}")
        } else {
            Ok(())
        }
    }
    pub fn add_tasks(
        &mut self,
        project: &crate::types::project::ProjectInfo,
        tasks: &[crate::types::task::Task],
    ) -> anyhow::Result<()> {
        self.import_task(
            &tasks
                .iter()
                .enumerate()
                .map(|(idx, t)| to_task(idx, project, t, None))
                .collect::<Vec<_>>(),
        )
    }
    pub fn start_task(
        &mut self,
        project: &crate::types::project::ProjectInfo,
        t: &crate::types::task::Task,
        start: chrono::DateTime<chrono::FixedOffset>,
    ) -> anyhow::Result<()> {
        self.import_task(&[to_task(0, project, t, Some(start))])
    }
    pub fn stop_task(
        &mut self,
        project: &crate::types::project::ProjectInfo,
        t: &crate::types::task::Task,
    ) -> anyhow::Result<()> {
        // task with start field set is considered active
        self.import_task(&[to_task(0, project, t, None)])
    }
    pub fn finish_task(
        &mut self,
        project: &crate::types::project::ProjectInfo,
        t: &crate::types::task::Task,
    ) -> anyhow::Result<()> {
        self.import_task(&[to_task(0, project, t, None)])
    }
    pub fn set_task_complete(
        &mut self,
        project: &crate::types::project::ProjectInfo,
        t: &crate::types::task::Task,
    ) -> anyhow::Result<()> {
        self.import_task(&[to_task(0, project, t, None)])
    }
}
