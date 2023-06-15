use std::fmt::{Display, Formatter};
use std::num::ParseIntError;
use std::str::FromStr;

use derive_more::{From, Deref, Add};
use serde::{Serialize, Deserialize};
use worker::RouteContext;

use super::Vin;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Add, Deref, From)]
pub struct SerialNumber(pub i32);


#[derive(Debug)]
pub enum SerialNumberParseError {
    NoLastElement,
    ParseIntError(ParseIntError),
}

impl From<ParseIntError> for SerialNumberParseError {
    fn from(error: ParseIntError) -> Self {
        SerialNumberParseError::ParseIntError(error)
    }
}

impl FromStr for SerialNumber {
    type Err = SerialNumberParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let last_element = s
            .split_whitespace()
            .last()
            .ok_or(SerialNumberParseError::NoLastElement)?;

        last_element
            .parse::<i32>()
            .map(SerialNumber)
            .map_err(Into::into)
    }
}

impl From<&std::string::String> for SerialNumber {
    fn from(serial: &std::string::String) -> Self {
        SerialNumber::from_str(serial).unwrap()
    }
}

impl Display for SerialNumber {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}


impl From<Vin> for SerialNumber {
    fn from(vin: Vin) -> Self {
        // SerialNumber is last 6 digits (0 padded) of Vin
        SerialNumber::from(&vin.0[11..].into())
    }
}

pub async fn highest_serial(ctx: &RouteContext<()>) -> SerialNumber {
    let d1 = ctx.env.d1("failcat_db").expect("Couldn't get db");
    let statement = d1.prepare("SELECT max(serial_number) FROM cars");
    let rows = statement
        .first::<i32>(Some("max(serial_number)"))
        .await
        .expect("Couldn't get rows");
    match rows {
        Some(row) => SerialNumber(row),
        None => SerialNumber(0),
    }
}