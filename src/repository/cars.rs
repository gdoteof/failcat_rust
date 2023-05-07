use serde::{Deserialize, Serialize};
use std::{collections::HashMap, result};
use worker::*;
use wasm_bindgen::JsValue;

use crate::models::{Car, CarId};

use super::*;

pub struct CarRepository {
    repository: Repository,
}

impl CarRepository {
    pub fn new() -> Self {
        Self {
            repository: Repository::new("cars"),
        }
    }

    pub async fn create_car(&self, ctx: &RouteContext<()>, car: Car) -> worker::Result<(CarId)> {
        Car::to_d1(&car, ctx).await
    }

    pub async fn get_car_by_id(
        &self,
        ctx: &RouteContext<()>,
        id: CarId,
    ) -> worker::Result<Option<Car>> {
        Car::from_d1(id, ctx).await
    }
}