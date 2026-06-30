use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ExternalTools {}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectRef {
    pub id: Uuid,
    pub root: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub id: Uuid,
    pub name: String,
    /// root project (e.g. "util" in project "util/task")
    pub root: String,
    #[serde(default)]
    pub description: Option<String>,
    pub created: DateTime<FixedOffset>,
    pub color: String,
    #[serde(default)]
    pub data: serde_json::Value,
    #[serde(default)]
    pub external_tools: ExternalTools,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectDef<T = super::task::Task> {
    pub project: ProjectInfo,
    pub tasks: Vec<T>,
}
