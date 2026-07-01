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
pub struct SuperProductivityState {
    pub id: String,
}
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ExternalState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub super_productivity: Option<SuperProductivityState>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TagRef {
    pub id: Uuid,
    pub name: String,
    #[serde(default)]
    pub external_tools: ExternalTools,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct TagInfo {
    pub id: Uuid,
    pub name: String,
    pub created: DateTime<FixedOffset>,
    pub color: String,
    #[serde(default)]
    pub data: serde_json::Value,
    #[serde(default)]
    pub external_tools: ExternalTools,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct TagState {
    #[serde(default)]
    pub external_tools: ExternalState,
}

pub type TagsState = BTreeMap<Uuid, TagState>;
