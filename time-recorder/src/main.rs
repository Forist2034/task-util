use std::{borrow::Cow, io::Write, os::unix::fs::OpenOptionsExt, path::PathBuf};

use anyhow::Context;
use chrono::{DateTime, Datelike, FixedOffset};
use clap::Parser;
use serde::{Deserialize, Serialize, ser::SerializeMap};
use uuid::Uuid;

#[derive(Deserialize)]
struct TaskData<'a> {
    task_id: Uuid,
    task_name: &'a str,
    description: &'a str,
    categories: Vec<&'a str>,
    /// task specific tags
    tags: Vec<&'a str>,
    timew_tags: Vec<&'a str>,
    value: serde_json::Value,
}

#[derive(Serialize, Deserialize)]
struct StartData {
    id: Uuid,
    start_time: DateTime<FixedOffset>,
}

#[derive(Serialize)]
struct RecordDataValue<'a> {
    task_id: Uuid,
    task_name: &'a str,
    value: serde_json::Value,
}
struct RecordData<'a>(RecordDataValue<'a>);
impl<'a> Serialize for RecordData<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut ser = serializer.serialize_map(Some(1))?;
        ser.serialize_entry(&self.0.task_id, &self.0)?;
        ser.end()
    }
}

#[derive(Serialize)]
struct Record<'a> {
    id: Uuid,
    start_time: DateTime<FixedOffset>,
    stop_time: DateTime<FixedOffset>,
    description: &'a str,
    categories: Vec<&'a str>,
    tags: Vec<&'a str>,
    data: RecordData<'a>,
}

#[derive(clap::Subcommand)]
enum Cmd {
    Start,
    Stop {
        #[arg(long)]
        stop_time: Option<String>,
        task: String,
        #[arg(last = true)]
        params: Vec<String>,
    },
}

#[derive(serde::Serialize)]
struct TimewData<'a> {
    id: u8,
    start: String,
    end: String,
    tags: Vec<Cow<'a, str>>,
    annotation: &'a str,
}

#[derive(clap::Parser)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

fn start() -> anyhow::Result<()> {
    let ts = std::time::SystemTime::now();
    let start_time = chrono::DateTime::<chrono::Local>::from(ts).fixed_offset();
    let id = uuid::Uuid::new_v7({
        let dur = ts
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap();
        uuid::Timestamp::from_unix(uuid::NoContext, dur.as_secs(), dur.subsec_nanos())
    });
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o440)
        .open(".started_task")
        .context("failed to create temp file")?;
    file.write_all(&serde_json::to_vec(&StartData { id, start_time }).unwrap())?;
    println!(
        "task {id} started at {start_time} ({})",
        start_time.to_rfc3339()
    );
    Ok(())
}
fn stop(stop_time: &Option<String>, task: &str, params: &[String]) -> anyhow::Result<()> {
    let stop_time = match stop_time {
        Some(st) => {
            chrono::DateTime::<FixedOffset>::parse_from_rfc3339(st).context("invalid stop time")?
        }
        None => chrono::Local::now().fixed_offset(),
    };
    println!("task stopped at {stop_time} ({})", stop_time.to_rfc3339());
    let start: StartData = serde_json::from_slice(
        &std::fs::read(".started_task").context("failed to read started task")?,
    )
    .context("failed to deserialize started task")?;
    let data = std::process::Command::new("nickel")
        .args(["export", "-f", "json", task, "--"])
        .args(params)
        .output()
        .context("failed to get task info")?;
    if !data.status.success() {
        anyhow::bail!(
            "nickel returns {:?}:\n{}",
            data.status,
            data.stderr.escape_ascii()
        );
    }
    let info: TaskData = serde_json::from_slice(&data.stdout).context("invalid task data")?;

    let timew_data = {
        let mut id_buf = [0; uuid::fmt::Hyphenated::LENGTH];
        serde_json::to_vec(std::slice::from_ref(&TimewData {
            id: 0,
            start: start
                .start_time
                .to_utc()
                .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            end: stop_time
                .to_utc()
                .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            tags: {
                let mut ret = Vec::from([
                    Cow::Borrowed(info.description),
                    format!("task:{}", info.task_name).into(),
                    format!("task_id:{}", info.task_id).into(),
                ]);
                ret.extend(info.categories.iter().map(|c| Cow::Borrowed(*c)));
                ret.extend(info.tags.iter().map(|t| Cow::Borrowed(*t)));
                ret.extend(info.timew_tags.iter().map(|t| Cow::Borrowed(*t)));
                ret
            },
            annotation: start.id.as_hyphenated().encode_lower(&mut id_buf),
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
        .write_all(&timew_data)
        .context("failed to write timew input")?;
    let status = timew.wait().context("failed to wait timew")?;
    if !status.success() {
        anyhow::bail!("timewarrior returns {status:?}");
    }

    let path = PathBuf::from({
        let y = start.start_time.year();
        format!(
            "{y}/{y}-{:02}/{:02}/{}.json",
            start.start_time.month(),
            start.start_time.day(),
            start.id
        )
    });
    std::fs::create_dir_all(path.parent().unwrap()).context("failed to create dir")?;
    std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o440)
        .open(path)
        .context("failed to create task file")?
        .write_all(
            &serde_json::to_vec_pretty(&Record {
                id: start.id,
                start_time: start.start_time,
                stop_time,
                description: info.description,
                categories: info.categories,
                tags: info.tags,
                data: RecordData(RecordDataValue {
                    task_id: info.task_id,
                    task_name: info.task_name,
                    value: info.value,
                }),
            })
            .unwrap(),
        )
        .context("failed to write task file")?;
    std::fs::remove_file(".started_task").context("failed to remove task file")?;

    Ok(())
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Start => start(),
        Cmd::Stop {
            stop_time,
            task,
            params,
        } => stop(&stop_time, &task, &params),
    }
}
