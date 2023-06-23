use serde::{Deserialize, Serialize};
use worker::{Database, RouteContext, console_log};

use super::{CarRepository, CarQuery, Car};


#[derive(Debug, Deserialize, Serialize, Clone,  PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Dealer {
    pub dealer_code: String,
    pub address: String,
    pub zip: String,
    pub car_count: i32, 
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

    pub async fn create(&self, dealer: &Dealer) -> worker::Result<()> {
        let statement = self.d1.prepare(
            "INSERT INTO dealers (dealer_code, address, zip, car_count) VALUES (?, ?, ?, ?)",
        );
        let _query = statement.bind(&[
            dealer.dealer_code.clone().into(),
            dealer.address.clone().into(),
            dealer.zip.clone().into(),
            dealer.car_count.into(),
        ])?;
        Ok(())
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
            dealer_code,
            address,
            zip,
            car_count: 0, // Initialize with 0, update when needed
        }
    }

    pub async fn from_d1(
        dealer_code: &str,
        d1: Database,
    ) -> worker::Result<Option<Self>> {
        let statement = d1.prepare("SELECT * FROM dealers WHERE dealer_code = ?");
        console_log!("dealer_code: {}", dealer_code);
        let query = statement.bind(&[dealer_code.into()])?;
        console_log!("query: {:?}", query);

        match query.all().await {
            Ok(result) => {
                console_log!("result: {:?}", result);
                let result = result.results::<Dealer>()?;
                Ok(Some(result[0].clone()))
            },
            Err(e) => {
                console_log!("error: {:?}", e);
                Ok(None)
            }
        }
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

    pub async fn cars(&self, ctx: &RouteContext<()>) -> worker::Result<Vec<Car>> {
        let car_query = CarQuery {
            dealer: Some(self.dealer_code.clone()),
            perpage: Some(500),
            ..Default::default()
        };
        let cars = CarRepository::new(ctx.env.d1("failcat_db").unwrap())
            .get_all_paginated(car_query)
            .await;
        match cars {
            Ok(cars) => Ok(cars),
            Err(e) => {
                console_log!("Error: {:?}", e);
                Err(e)
            }
        }
    }
}