#![allow(dead_code)]
use chrono::Utc;
use derive_more::{Deref, Display, From};
use std::{
    num::ParseIntError,
};

use serde::{Deserialize, Serialize};
use worker::*;

pub mod car;
pub use car::*;
pub mod serial;
pub use serial::*;
pub mod dealer;
pub use dealer::*;

#[derive(
    Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord, Clone, Display, From, Deref,
)]
pub struct Vin(pub String);


#[derive(
    Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Display, From, Deref,
)]
pub struct CarId(pub i32);



#[serde(rename_all = "lowercase")]
#[derive(Serialize, Deserialize, Debug, Default)]
pub enum CarOrder {
    Id,
    #[default]
    Serial,
}




#[derive(Debug)]
pub enum CarQueryError {
    ParseIntError(ParseIntError),
    ParseSerialError, // Replace with the actual error type from SerialNumber::from_str
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
                sql += "sold_to = ? AND ";
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
        // Strange bug(?) where the order by clause is not working with the prepared statement
        sql += match query.order {
            Some(CarOrder::Id) => " ORDER BY id DESC LIMIT ? OFFSET ?",
            _ => " ORDER BY serial_number DESC LIMIT ? OFFSET ?",
        };

        bindings.push(query.perpage.unwrap_or(10).into());
        bindings.push(query.offset.unwrap_or(0).into());

        console_debug!("SQL: {}", sql);
        console_debug!("Bindings: {:?}", bindings);
        let statement = self.d1.prepare(&sql);
        let query = statement.bind(&bindings)?;
        console_debug!("query: {:?}", query);
        let d1_result_result = query.all().await;
        let d1_result = d1_result_result?.results()?;

        Ok(d1_result)
    }
}
#[derive(Serialize, Deserialize, Debug, Default)]
pub struct CarQuery {
    pub dealer: Option<String>,
    pub perpage: Option<i32>,
    pub offset: Option<i32>,
    pub order: Option<CarOrder>,
    pub minimum_serial: Option<SerialNumber>,
    pub maximum_serial: Option<SerialNumber>,
    pub minimum_id: Option<SerialNumber>,
    pub maximum_maximum: Option<SerialNumber>,
}


impl From<ParseIntError> for CarQueryError {
    fn from(err: ParseIntError) -> CarQueryError {
        CarQueryError::ParseIntError(err)
    }
}



