use sea_orm::DeriveEntityModel;
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

impl From<AlarmInitRespItem> for Alarm {
    fn from(value: AlarmInitRespItem) -> Self {
        Self {
            house_code: value.house_code,
            tenant_id: Default::default(),
            farm_id: value.farm_id,
            target_name: value.target_name,
            alarm_item: value.alarm_item,
            content: value.content,
            timestamp: value.alarm_time,
            received_time: Some(value.alarm_time),
            alarm_type: value.alarm_type,
            is_confirmed: false,
            is_test: false,
            is_alarm: true,
            day_age: value.day_age,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlarmsInitResp {
    pub total_count: u32,
    pub items: Vec<AlarmInitRespItem>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AlarmInitRespItem {
    pub farm_id: Option<String>,
    pub farm_name: Option<String>,
    pub location: Option<String>,
    pub house_code: String,
    #[serde(rename = "TimeStamp", with = "rfc3339_time")]
    pub alarm_time: OffsetDateTime,
    pub day_age: Option<u32>,
    pub target_name: String,
    pub alarm_item: String,
    pub alarm_type: String,
    pub content: String,
}

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "SysHouse")]
pub struct SysHouse {
    pub id: uuid,
}
