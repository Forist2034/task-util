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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    #[default]
    Pending,
    Completed,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskState {
    pub status: Status,
    #[serde(default)]
    pub completed: Option<DateTime<FixedOffset>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Task {
    pub id: Uuid,
    pub name: String,
    #[serde(default)]
    pub status: Status,
    #[serde(default)]
    pub description: Option<String>,
    pub created: DateTime<FixedOffset>,
    #[serde(default)]
    pub completed: Option<DateTime<FixedOffset>>,
    pub project: super::project::ProjectRef,
    #[serde(default)]
    pub tags: Vec<super::tag::TagRef>,
    #[serde(default)]
    pub data: serde_json::Value,
    #[serde(default)]
    pub external_tools: ExternalTools,
}
