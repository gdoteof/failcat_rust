#![allow(dead_code)]
use chrono::Utc;
use derive_more::{Deref, Display, From};
use std::{
    fmt::{Display, Formatter},
    ops::Add, num::ParseIntError, collections::HashMap,
};

use serde::{Deserialize, Serialize};
use worker::*;

pub mod car;
pub use car::*;

#[derive(
    Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord, Clone, Display, From, Deref,
)]
pub struct Vin(pub String);
#[derive(
    Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Display, From, Deref,
)]
pub struct CarId(pub i32);
#[derive(
    Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, From, Deref,
)]
pub struct SerialNumber(pub i32);
impl Display for SerialNumber {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
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
                .unwrap_or_else(|_| panic!("Could not parse ->>{}<<-", serial)),
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
    let rows = statement
        .first::<i32>(Some("max(serial_number)"))
        .await
        .expect("Couldn't get rows");
    match rows {
        Some(row) => SerialNumber(row),
        None => SerialNumber(0),
    }
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
        match query.first(None).await? {
            Some(ScraperLog { id, .. }) => Ok(id.unwrap()),
            None => Err("No scraper log found".into()),
        }
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

    pub async fn from_d1(
        model_code: &str,
        ctx: &RouteContext<()>,
    ) -> worker::Result<Option<CarModel>> {
        let d1 = ctx.env.d1("failcat_db").expect("Couldn't get db");
        let statement = d1.prepare("SELECT * FROM car_models WHERE model_code = ?");
        let query = statement.bind(&[model_code.into()])?;
        let result = query.first::<CarModel>(None).await?;
        Ok(result)
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Dealer {
    id: Option<i32>,
    dealer_code: String,
    address: String,
    zip: String,
    car_count: i32, // Aggregated value, can be calculated when needed
                    // cars relationship is omitted here, but can be implemented if needed
}

pub struct DealerRepository {
    d1: Database,
}

impl DealerRepository {
    pub fn new(d1: Database) -> Self {
        DealerRepository { d1 }
    }

    pub async fn get(&self, dealer_code: &str) -> worker::Result<Option<Dealer>> {
        let statement = self
            .d1
            .prepare("SELECT * FROM dealers WHERE dealer_code = ?");
        let query = statement.bind(&[dealer_code.into()])?;
        let result = query.first::<Dealer>(None).await?;
        Ok(result)
    }

    pub async fn create(&self, dealer: &Dealer) -> worker::Result<i32> {
        let statement = self.d1.prepare(
            "INSERT INTO dealers (dealer_code, address, zip, car_count) VALUES (?, ?, ?, ?)",
        );
        let query = statement.bind(&[
            dealer.dealer_code.clone().into(),
            dealer.address.clone().into(),
            dealer.zip.clone().into(),
            dealer.car_count.into(),
        ])?;
        match query.first(None).await? {
            Some(Dealer { id, .. }) => Ok(id.unwrap()),
            None => Err("No dealer found".into()),
        }
    }

    pub async fn get_all(&self) -> worker::Result<Vec<Dealer>> {
        let statement = self.d1.prepare("SELECT * FROM dealers");
        let d1_result = statement.all().await?;
        let result = d1_result.results::<Dealer>()?;
        Ok(result)
    }
}

impl Dealer {
    pub fn new(dealer_code: String, address: String, zip: String) -> Self {
        Dealer {
            id: None,
            dealer_code,
            address,
            zip,
            car_count: 0, // Initialize with 0, update when needed
        }
    }

    pub async fn from_d1(
        dealer_code: &str,
        ctx: &RouteContext<()>,
    ) -> worker::Result<Option<Self>> {
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

pub struct CarRepository {
    pub d1: Database,
}

impl CarRepository {
    pub fn new(d1: Database) -> Self {
        CarRepository { d1 }
    }

    // pub async fn get_all_paginated_old(&self, query: CarQuery) -> worker::Result<Vec<Car>> {
    //     let order_by = match query.order {
    //         Id => "id",
    //         Serial => "serial_number",
    //     };
    //     let statement = self
    //         .d1
    //         .prepare("SELECT * FROM cars ORDER BY ? DESC LIMIT ? OFFSET ? ");
    //     let query = statement.bind(&[order_by.into(), query.page_size.into(), ((page - 1) * page_size).into()])?;
    //     let d1_result_result = query.all().await;
    //     let d1_result = d1_result_result?.results()?;
    //     Ok(d1_result)
    // }
    pub async fn get_all_paginated(&self, query: CarQuery) -> worker::Result<Vec<Car>> {
        let order_by = match query.order {
            CarOrder::Id => "id",
            CarOrder::Serial => "serial_number",
        };

        let mut sql = "SELECT * FROM cars".to_string();
        let mut bindings = vec![];

        if query.dealer.is_some()
            || query.minimum_serial.is_some()
            || query.maximum_serial.is_some()
            || query.minimum_id.is_some()
            || query.maximum_maximum.is_some()
        {
            sql += " WHERE ";

            if let Some(dealer) = &query.dealer {
                sql += "dealer = ? AND ";
                bindings.push(dealer.into());
            }

            if let Some(minimum_serial) = &query.minimum_serial {
                sql += "serial_number >= ? AND ";
                bindings.push(minimum_serial.0.into());
            }

            if let Some(maximum_serial) = &query.maximum_serial {
                sql += "serial_number <= ? AND ";
                bindings.push(maximum_serial.0.into());
            }

            if let Some(minimum_id) = &query.minimum_id {
                sql += "id >= ? AND ";
                bindings.push(minimum_id.0.into());
            }

            if let Some(maximum_id) = &query.maximum_maximum {
                sql += "id <= ? AND ";
                bindings.push(maximum_id.0.into());
            }

            // Remove the trailing " AND "
            sql = sql[0..sql.len() - 5].to_string();
        }

        sql += " ORDER BY ? DESC LIMIT ? OFFSET ?";
        bindings.push(order_by.into());
        bindings.push(query.per_page.into());
        bindings.push(query.offset.into());

        console_debug!("SQL: {}", sql);
        console_debug!("Bindings: {:?}", bindings);
        let statement = self.d1.prepare(&sql);
        let query = statement.bind(&bindings)?;
        let d1_result_result = query.all().await;
        let d1_result = d1_result_result?.results()?;

        Ok(d1_result)
    }
}
#[derive(Debug)]
pub struct CarQuery {
    pub dealer: Option<String>,
    pub per_page: i32,
    pub offset: i32,
    pub order: CarOrder,
    pub minimum_serial: Option<SerialNumber>,
    pub maximum_serial: Option<SerialNumber>,
    pub minimum_id: Option<SerialNumber>,
    pub maximum_maximum: Option<SerialNumber>,
}

#[derive(Debug)]
pub enum CarOrder {
    Id,
    Serial,
}

#[derive(Debug)]
pub enum CarQueryError {
    ParseIntError(ParseIntError),
    ParseSerialError, // Replace with the actual error type from SerialNumber::from_str
}

impl From<ParseIntError> for CarQueryError {
    fn from(err: ParseIntError) -> CarQueryError {
        CarQueryError::ParseIntError(err)
    }
}

// Add similar impl block for the error type from SerialNumber::from_str

impl CarQuery {
    pub fn from_hashmap(hashmap: HashMap<String, String>) -> Result<Self> {
        let dealer = hashmap.get("dealer").cloned();
        let per_page = hashmap.get("per_page").map_or(Ok(10), |v| v.parse::<i32>()).unwrap();
        let offset = hashmap.get("offset").map_or(Ok(0), |v| v.parse::<i32>()).unwrap();
        let order = CarOrder::Serial;
        let minimum_serial = hashmap.get("minimum_serial").map(|s| SerialNumber::from_str(s));
        let maximum_serial = hashmap.get("maximum_serial").map(|s| SerialNumber::from_str(s));
        let minimum_id = hashmap.get("minimum_id").map(|s| SerialNumber::from_str(s));
        let maximum_maximum = hashmap.get("maximum_maximum").map(|s| SerialNumber::from_str(s));


        let result = Ok(CarQuery {
            dealer,
            per_page,
            offset,
            order,
            minimum_serial,
            maximum_serial,
            minimum_id,
            maximum_maximum,
        });

        console_debug!("result: {:?}", result);
        result
    }
}
