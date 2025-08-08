use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Alarm {
    pub tenant_id: Option<String>,
    pub farm_id: Option<String>,
    pub target_name: String,
    pub alarm_item: String,
    pub content: String,
    #[serde(rename = "TimeStamp")]
    pub timestamp: OffsetDateTime,
    pub alarm_type: String,
    pub is_alarm: bool,
    #[serde(default)]
    pub day_age: u32,
}
