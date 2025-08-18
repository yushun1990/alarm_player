use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

mod rfc3339_time {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use time::{OffsetDateTime, format_description::well_known::Rfc3339};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<OffsetDateTime, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        OffsetDateTime::parse(&s, &Rfc3339).map_err(serde::de::Error::custom)
    }

    pub fn serialize<S>(date: &OffsetDateTime, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        date.format(&Rfc3339)
            .map_err(serde::ser::Error::custom)?
            .serialize(serializer)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Alarm {
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
    #[serde(skip)]
    #[serde(default)]
    pub is_new: bool,
    #[serde(default)]
    pub day_age: u32,
}

impl Default for Alarm {
    fn default() -> Self {
        Self {
            tenant_id: Default::default(),
            farm_id: Default::default(),
            target_name: Default::default(),
            alarm_item: Default::default(),
            content: "测试报警".into(),
            timestamp: OffsetDateTime::now_utc(),
            received_time: Some(OffsetDateTime::now_utc()),
            alarm_type: "test".to_string(),
            is_test: true,
            is_alarm: true,
            is_new: Default::default(),
            day_age: Default::default(),
        }
    }
}
