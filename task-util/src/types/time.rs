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
pub struct TimeData {
    pub type_name: String,
    pub type_id: Uuid,
    #[serde(default)]
    pub data: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TimeRecord<P, T> {
    pub id: Uuid,
    pub start_time: DateTime<FixedOffset>,
    pub end_time: DateTime<FixedOffset>,
    pub project: P,
    pub task: T,
    pub done: bool,
    pub data: Option<TimeData>,
    #[serde(default)]
    pub external_tools: ExternalTools,
}
