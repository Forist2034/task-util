use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ExternalTools {}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectRef {
    pub id: Uuid,
    /// root project (e.g. "util" in project "util/task")
    pub root: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub id: Uuid,
    pub name: String,
    pub created: DateTime<FixedOffset>,
    #[serde(default)]
    pub data: serde_json::Value,
    #[serde(default)]
    pub external_tools: ExternalTools,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectDef {
    pub project: ProjectInfo,
    pub tasks: Vec<super::task::Task>,
}
