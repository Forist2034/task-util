use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Timewarrior {
    pub extra_tags: Vec<String>,
}
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ExternalTools {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timewarrior: Option<Timewarrior>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TimeInfo {
    #[serde(default)]
    pub data: serde_json::Value,
    #[serde(default)]
    pub external_tools: ExternalTools,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TimeRecord {
    pub id: Uuid,
    pub start_time: DateTime<FixedOffset>,
    pub end_time: DateTime<FixedOffset>,
    pub project: super::project::ProjectInfo,
    pub task: super::task::Task,
    pub done: bool,
    pub data: serde_json::Value,
    pub external_tools: ExternalTools,
}
