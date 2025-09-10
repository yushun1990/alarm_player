use serde::{Deserialize, Deserializer, Serialize, Serializer};
use time::{PrimitiveDateTime, format_description::well_known::Iso8601};

#[allow(unused)]
pub fn deserialize<'de, D>(deserializer: D) -> Result<PrimitiveDateTime, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    PrimitiveDateTime::parse(&s, &Iso8601::DATE_TIME).map_err(serde::de::Error::custom)
}

pub fn serialize<S>(date: &PrimitiveDateTime, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    date.format(&Iso8601::DATE_TIME)
        .map_err(serde::ser::Error::custom)?
        .serialize(serializer)
}
