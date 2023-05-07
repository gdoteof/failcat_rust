use chrono::{DateTime, Datelike, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use worker::*;

#[derive(Debug, Deserialize, Serialize)]
pub struct Vin(pub String);
#[derive(Debug, Deserialize, Serialize)]
pub struct CarId(pub i32);
#[derive(Debug, Deserialize, Serialize)]
pub struct SerialNumber(pub i32);

#[derive(Debug, Deserialize, Serialize)]
pub struct Car {
    id: Option<CarId>,
    vin: Vin,
    ext_color: String,
    int_color: String,
    car_model: String,
    opt_code: String,
    ship_to: String,
    sold_to: String,
    created_date: String,
    serial_number: i32,
    model_year: String,
    dead_until: Option<String>,
    last_attempt: Option<String>,
}

impl Car {
    pub async fn from_d1(id: CarId, ctx: &RouteContext<()>) -> worker::Result<Option<Car>> {
        let d1 = ctx.env.d1("failcat_db").expect("Couldn't get db");
        let statement = d1.prepare("SELECT * FROM cars WHERE id = ?");
        let query = statement.bind(&[id.0.into()])?;
        let result = query.first::<Car>(None).await?;
        Ok(result)
    }

    pub async fn to_d1(&self, ctx: RouteContext<(Car)>) -> worker::Result<CarId> {
        let d1 = ctx.env.d1("failcat_db").expect("Couldn't get db");
        let statement = d1.prepare(
            "INSERT INTO cars (vin, ext_color, int_color, car_model, opt_code, ship_to, sold_to, created_date, serial_number, model_year, dead_until, last_attempt) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, now())",
        );
        let query = statement.bind(&[
            self.vin.0.clone().into(),
            self.ext_color.clone().into(),
            self.int_color.clone().into(),
            self.car_model.clone().into(),
            self.opt_code.clone().into(),
            self.ship_to.clone().into(),
            self.sold_to.clone().into(),
            self.created_date.clone().to_string().into(),
            self.serial_number.clone().into(),
            self.model_year.clone().into(),
            self.dead_until.clone().into(),
            self.last_attempt.clone().into(),
        ])?;
        return match query.first(None).await? {
            Some(Car { id, .. }) => Ok(id.unwrap()),
            None => Err(Error::RouteNoDataError),
        };
    }

    pub async fn from_kv(serial: &str, ctx: RouteContext<Car>) -> worker::Result<Option<Car>> {
        let kv = ctx.env.kv("failcat").expect("Couldn't get db");
        let response = kv.get(serial).json().await;
        match response {
            Ok(data) => {
                let result = data;
                Ok(result)
            }
            Err(_) => Ok(None),
        }
    }

    // Add a car to the KV store
    pub async fn to_kv(&self, ctx: RouteContext<Car>, _sql_id: CarId) -> worker::Result<()> {
        let kv = ctx.env.kv("failcat").expect("Couldn't get db");
        let response = kv
            .put(&self.serial_number.to_string(), &self)
            .expect("Couldn't build put options")
            .execute()
            .await;
        match response {
            Ok(_) => Ok(()),
            Err(_) => Ok(()),
        }
    }
}

pub struct VinScrape {
    pub vin: String,
    pub dealer: Dealer,
    pub car_model: CarModel,
    pub car: Car,
}

#[derive(Debug, Deserialize)]
pub struct Dealer {
    dealer_code: String,
    address: String,
    zip: String,
    car_count: i32,
}

#[derive(Debug, Deserialize)]
pub struct CarModel {
    model_code: String,
    description: String,
}
