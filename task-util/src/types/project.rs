use std::collections::BTreeMap;

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct SuperProductivity {
    #[serde(default)]
    pub id: Option<String>,
}
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ExternalTools {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub super_productivity: Option<SuperProductivity>,
}

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
pub struct SuperProductivityState {
    pub id: String,
}
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ExternalState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub super_productivity: Option<SuperProductivityState>,
}
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ProjectState {
    #[serde(default)]
    pub external_tools: ExternalState,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ProjectDefState {
    pub project: ProjectState,
    pub tasks: BTreeMap<Uuid, super::task::TaskState>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectDef {
    pub project: ProjectInfo,
    pub tasks: Vec<super::task::Task>,
}
impl ProjectDef {
    pub fn task_index(&self, task_id: &Uuid) -> Option<usize> {
        self.tasks
            .iter()
            .enumerate()
            .find_map(|(idx, t)| if &t.id == task_id { Some(idx) } else { None })
    }
}
