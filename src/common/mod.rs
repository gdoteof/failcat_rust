use chrono::{DateTime, Utc, TimeZone};
use serde::{Deserializer, Deserialize};


pub fn deserialize_string_to_datetime<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;

    Utc.datetime_from_str(&s, "%Y-%m-%d %H:%M:%S%.6f")
        .map_err(serde::de::Error::custom)
}