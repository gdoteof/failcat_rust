use chrono::{DateTime, Utc, TimeZone};
use serde::{Deserializer, Deserialize, Serialize};

use crate::models::{SerialNumber, CarId};


pub fn deserialize_string_to_datetime<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;

    Utc.datetime_from_str(&s, "%Y-%m-%d %H:%M:%S%.6f")
        .map_err(serde::de::Error::custom)
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ScrapeResponse {
    pub attempted: SerialNumber,
    pub found: Option<CarId>
}

impl ScrapeResponse {
    pub fn new(attempted: SerialNumber, found: Option<CarId>) -> Self {
        Self {
            attempted,
            found
        }
    }

    pub fn found(attempted: SerialNumber, found: CarId) -> Self {
        Self::new(attempted, Some(found))
    }

    pub fn not_found(attempted: SerialNumber) -> Self {
        Self::new(attempted, None)
    }
}

