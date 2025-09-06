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
