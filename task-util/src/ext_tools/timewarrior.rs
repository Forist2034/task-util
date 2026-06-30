use std::{borrow::Cow, io::Write};

use anyhow::Context as _;
use uuid::Uuid;

use super::taskwarrior::DateTime;

#[derive(serde::Serialize)]
struct TimewData<'a> {
    id: u8,
    start: DateTime,
    end: DateTime,
    tags: Vec<Cow<'a, str>>,
    annotation: Uuid,
}

pub fn import_record(
    project: &crate::types::project::ProjectInfo,
    t: &crate::types::time::TimeRecord,
) -> anyhow::Result<()> {
    let data = {
        serde_json::to_vec(std::slice::from_ref(&TimewData {
            id: 0,
            start: DateTime(t.start_time.to_utc()),
            end: DateTime(t.end_time.to_utc()),
            tags: {
                let mut ret = Vec::from([
                    Cow::Borrowed(t.task.name.as_str()),
                    format!("task_id:{}", t.task.id).into(),
                    format!("project:{}.{}", project.root, project.name).into(),
                ]);
                ret.extend(t.task.tags.iter().map(|t| Cow::Borrowed(t.name.as_str())));
                ret.extend(
                    t.task
                        .external_tools
                        .timewarrior
                        .as_ref()
                        .map_or::<&[String], _>(&[], |v| &v.extra_tags)
                        .iter()
                        .map(|t| Cow::Borrowed(t.as_str())),
                );
                ret.extend(
                    t.external_tools
                        .timewarrior
                        .as_ref()
                        .map_or::<&[String], _>(&[], |v| &v.extra_tags)
                        .iter()
                        .map(|t| Cow::Borrowed(t.as_str())),
                );
                ret
            },
            annotation: t.id,
        }))
        .unwrap()
    };
    let mut timew = std::process::Command::new("timew")
        .arg("import")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .context("failed to track info to timew")?;
    timew
        .stdin
        .take()
        .unwrap()
        .write_all(&data)
        .context("failed to write timew input")?;
    let status = timew.wait().context("failed to wait timew")?;
    if !status.success() {
        anyhow::bail!("timewarrior returns {status:?}");
    }
    Ok(())
}
