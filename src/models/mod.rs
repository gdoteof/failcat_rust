use std::{fmt::{Display, Formatter}, ops::Add};

use chrono::Utc;

use serde::{Deserialize, Serialize};
use worker::*;


pub mod car;
pub use car::*;

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Vin(pub String);
#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CarId(pub i32);
#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SerialNumber(pub i32);
impl Display for SerialNumber {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.to_string())
    }
}

impl SerialNumber {
    fn from_str(serial: &str) -> Self {
        SerialNumber(
            serial
                .split_whitespace()
                .last()
                .unwrap()
                .parse::<i32>()
                .expect(format!("Could not parse ->>{}<<-", serial).as_str()),
        )
    }
}

impl From<Vin> for SerialNumber {
    fn from(vin: Vin) -> Self {
        // SerialNumber is last 6 digits (0 padded) of Vin
        SerialNumber::from_str(&vin.0[11..])
    }
}

impl From<&std::string::String> for SerialNumber {
    fn from(serial: &std::string::String) -> Self {
        SerialNumber(
            serial
                .split_whitespace()
                .last()
                .unwrap()
                .parse::<i32>()
                .expect("Could not parse"),
        )
    }
}

impl Add<i32> for SerialNumber {
    type Output = Self;

    fn add(self, rhs: i32) -> Self::Output {
        SerialNumber(self.0 + rhs)
    }
}


pub async fn highest_serial(ctx: &RouteContext<()>) -> SerialNumber {
    let d1 = ctx.env.d1("failcat_db").expect("Couldn't get db");
    let statement = d1.prepare("SELECT max(serial_number) FROM cars");
    let rows = statement.first::<i32>(Some("max(serial_number)")).await.expect("Couldn't get rows");
    return match rows {
        Some(row) => SerialNumber(row),
        None => SerialNumber(0),
    }
}

pub struct VinScrape {
    pub vin: String,
    pub dealer: Dealer,
    pub car_model: CarModel,
    pub car: Car,
}


#[derive(Debug, Deserialize, Serialize)]
pub struct ScraperLog {
    id: Option<i32>,
    found_cars: i32,
    run_start: String,
    run_end: String,
    run_type: String,
    success: bool,
}

impl ScraperLog {
    pub fn new(found_cars: i32, run_start: String, run_type: String, success: bool) -> Self {
        ScraperLog {
            id: None,
            found_cars,
            run_start,
            run_end: Utc::now().to_rfc2822(),
            run_type,
            success,
        }
    }

    // Implement methods for interacting with the database here
    pub async fn from_d1(id: i32, ctx: &RouteContext<()>) -> worker::Result<Option<ScraperLog>> {
        let d1 = ctx.env.d1("failcat_db").expect("Couldn't get db");
        let statement = d1.prepare("SELECT * FROM scraper_logs WHERE id = ?");
        let query = statement.bind(&[id.into()])?;
        let result = query.first::<ScraperLog>(None).await?;
        Ok(result)
    }

    pub async fn to_d1(&self, ctx: &RouteContext<()>) -> worker::Result<i32> {
        let d1 = ctx.env.d1("failcat_db").expect("Couldn't get db");
        let statement = d1.prepare(
            "INSERT INTO scraper_logs (found_cars, run_start, run_end, run_type, success) VALUES (?, ?, ?, ?, ?)",
        );
        let query = statement.bind(&[
            self.found_cars.into(),
            self.run_start.clone().into(),
            self.run_end.clone().into(),
            self.run_type.clone().into(),
            self.success.into(),
        ])?;
        return match query.first(None).await? {
            Some(ScraperLog { id, .. }) => Ok(id.unwrap()),
            None => Err("No scraper log found".into()),
        };
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CarModel {
    model_code: String,
    description: String,
    // car relationship is omitted here, but can be implemented if needed
}

impl CarModel {
    pub fn new(model_code: String, description: String) -> Self {
        CarModel {
            model_code,
            description,
        }
    }

    pub async fn from_d1(model_code: &str, ctx: &RouteContext<()>) -> worker::Result<Option<CarModel>> {
        let d1 = ctx.env.d1("failcat_db").expect("Couldn't get db");
        let statement = d1.prepare("SELECT * FROM car_models WHERE model_code = ?");
        let query = statement.bind(&[model_code.into()])?;
        let result = query.first::<CarModel>(None).await?;
        Ok(result)
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Dealer {
    dealer_code: String,
    address: String,
    zip: String,
    car_count: i32, // Aggregated value, can be calculated when needed
    // cars relationship is omitted here, but can be implemented if needed
}

impl Dealer {
    pub fn new(dealer_code: String, address: String, zip: String) -> Self {
        Dealer {
            dealer_code,
            address,
            zip,
            car_count: 0, // Initialize with 0, update when needed
        }
    }

    pub async fn from_d1(dealer_code: &str, ctx: &RouteContext<()>) -> worker::Result<Option<Self>> {
        let d1 = ctx.env.d1("failcat_db").expect("Couldn't get db");
        let statement = d1.prepare("SELECT * FROM dealers WHERE dealer_code = ?");
        let query = statement.bind(&[dealer_code.into()])?;
        let result = query.first::<Self>(None).await?;
        Ok(result)
    }

    pub async fn to_d1(&self, ctx: &RouteContext<()>) -> worker::Result<()> {
        let d1 = ctx.env.d1("failcat_db").expect("Couldn't get db");
        let statement = d1.prepare(
            "INSERT INTO dealers (dealer_code, address, zip, car_count) VALUES ($1, $2, $3, $4)",
        );
        let query = statement.bind(&[
            self.dealer_code.clone().into(),
            self.address.clone().into(),
            self.zip.clone().into(),
            self.car_count.into(),
        ])?;
        query.first::<Self>(None).await?;
        Ok(())
    }
}
