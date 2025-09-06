use crate::util::rfc3339_time;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Alarm {
    #[serde(skip)]
    #[serde(default)]
    pub house_code: String,
    pub tenant_id: Option<String>,
    pub farm_id: Option<String>,
    pub target_name: String,
    pub alarm_item: String,
    pub content: String,
    #[serde(rename = "TimeStamp", with = "rfc3339_time")]
    pub timestamp: OffsetDateTime,
    #[serde(skip)]
    #[serde(default)]
    pub received_time: Option<OffsetDateTime>,
    pub alarm_type: String,
    #[serde(default)]
    pub is_test: bool,
    pub is_alarm: bool,
    // 报警确认状态, 已确认报警不播放
    #[serde(skip)]
    #[serde(default)]
    pub is_confirmed: bool,
    pub day_age: Option<u32>,
}

impl Default for Alarm {
    fn default() -> Self {
        Self {
            house_code: Default::default(),
            tenant_id: Default::default(),
            farm_id: Default::default(),
            target_name: Default::default(),
            alarm_item: Default::default(),
            content: "测试报警".into(),
            timestamp: OffsetDateTime::now_utc(),
            received_time: Some(OffsetDateTime::now_utc()),
            alarm_type: "test".to_string(),
            is_confirmed: false,
            is_test: true,
            is_alarm: true,
            day_age: Default::default(),
        }
    }
}
