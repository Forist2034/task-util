use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ExternalTools {}

#[derive(Debug, Serialize, Deserialize)]
pub struct TagRef {
    pub id: Uuid,
    pub name: String,
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
