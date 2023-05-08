use crate::scraper::vinlookup::{self, get_possible_vins_from_serial};
use chrono::{DateTime, Utc};
use worker::wasm_bindgen::JsValue; // Add Fixed to imports

use super::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, PartialEq, PartialOrd)]
pub struct Car {
    pub id: Option<CarId>,
    pub vin: Vin,
    pub ext_color: String,
    pub int_color: String,
    pub car_model: String,
    pub opt_code: String,
    pub ship_to: String,
    pub sold_to: String,
    pub created_date: DateTime<Utc>,
    pub serial_number: SerialNumber,
    pub model_year: String,
    pub dead_until: Option<String>,
    pub last_attempt: Option<String>,
}

impl Car {
    pub fn new(
        vin: Vin,
        ext_color: String,
        int_color: String,
        car_model: String,
        opt_code: String,
        ship_to: String,
        sold_to: String,
        created_on: DateTime<Utc>,
        serial_number: SerialNumber,
        model_year: String,
        dead_until: Option<String>,
        last_attempt: Option<String>,
    ) -> Self {
        Self {
            id: None,
            vin,
            ext_color,
            int_color,
            car_model,
            opt_code,
            ship_to,
            sold_to,
            created_date: Utc::now(),
            serial_number,
            model_year,
            dead_until: None,
            last_attempt: None,
        }
    }

    pub fn set_id(&mut self, id: CarId) {
        self.id = Some(id);
    }

    pub async fn from_d1(id: CarId, ctx: &RouteContext<()>) -> worker::Result<Option<Car>> {
        let d1 = ctx.env.d1("failcat_db").expect("Couldn't get db");
        let statement = d1.prepare("SELECT * FROM cars WHERE id = ?");
        let query = statement.bind(&[id.0.into()])?;
        let result = query.first::<Car>(None).await?;
        Ok(result)
    }

    pub async fn from_d1_serial(
        serial_number: SerialNumber,
        ctx: &RouteContext<()>,
    ) -> worker::Result<Option<CarId>> {
        console_debug!("Entering from_d1_serial with:{}", serial_number);
        let d1 = ctx.env.d1("failcat_db").expect("Couldn't get db");
        let statement = d1.prepare("SELECT * FROM cars WHERE serial_number = ?");
        console_debug!("statement prepared {:?}", statement);
        let query = statement.bind(&[serial_number.0.into()]);
        console_debug!("query bound {:?}", query);
        return match query {
            Ok(q) => {
                console_debug!("query ok");
                let result = q.first::<CarId>("id".into()).await;
                console_debug!("got result from d1: {:?}", result);
                match result {
                    Ok(r) => {
                        console_debug!("result ok");
                        Ok(r)
                    }
                    Err(e) => {
                        console_debug!("result error: {:?}", e);
                        Err(e)
                    }
                }
            }
            Err(e) => {
                console_debug!("query error: {:?}", e);
                Err(e)
            }
        };
    }

    pub async fn to_d1(&self, ctx: &RouteContext<()>) -> worker::Result<CarId> {
        console_debug!("attempting to write to D1 with:\n{:?}", self);
        let d1 = ctx.env.d1("failcat_db").expect("Couldn't get db");
        let car_id = Car::from_d1_serial(self.serial_number.clone(), ctx).await;
        match car_id {
            Ok(Some(id)) => {
                console_debug!("Car already exists in db with id: {:?}", id);
                return Ok(id);
            }
            Ok(None) => (),
            Err(e) => return Err(e),
        }
        let statement = d1.prepare(
            "INSERT INTO cars (vin, ext_color, int_color, car_model, opt_code, ship_to, sold_to, created_date, serial_number, model_year, dead_until, last_attempt) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);",
        );

        let created_date = self
            .created_date
            .clone()
            .format("%Y-%m-%d %H:%M:%S%.6f")
            .to_string();

        let bind_list: [JsValue; 12] = [
            self.vin.0.clone().into(),
            self.ext_color.clone().into(),
            self.int_color.clone().into(),
            self.car_model.clone().into(),
            self.opt_code.clone().into(),
            self.ship_to.clone().into(),
            self.sold_to.clone().into(),
            created_date.into(),
            self.serial_number.0.into(),
            self.model_year.clone().into(),
            Utc::now().to_string().into(),
            Utc::now().to_string().into(),
        ];

        let maybe_statement = statement.bind(&bind_list);
        console_debug!("bind list {:?}", bind_list);
        console_debug!("statement bound {:?}", maybe_statement);

        match maybe_statement {
            Ok(statement) => {
                console_debug!("\n\nInserting car into db with statement");
                return match statement.first::<()>(None).await {
                    Ok(None) => {
                        let car_id = Car::from_d1_serial(self.serial_number.clone(), ctx)
                            .await?
                            .expect("Couldn't find car we just saved");
                        Ok(car_id)
                    }
                    Ok(Some(something)) => Err(format!(
                        "\n\nError inserting car into db unexpected: {:?}",
                        something
                    )
                    .into()),
                    Err(e) => Err(format!("\n\nError inserting car into db: {:?}", e).into()),
                };
            }
            Err(e) => {
                console_debug!("\n\nActually binding failed with: {:?}", e);
                return Err(e);
            }
        }
    }

    pub async fn from_kv(
        serial: SerialNumber,
        ctx: &RouteContext<()>,
    ) -> worker::Result<Option<Car>> {
        let kv = ctx.env.kv("vinscrapes").expect("Couldn't get db");
        let response = kv.get(&serial.to_string()).json().await;
        match response {
            Ok(data) => {
                let result = data;
                Ok(result)
            }
            Err(_) => Ok(None),
        }
    }

    // Add a car to the KV store
    pub async fn to_kv(
        &self,
        ctx: &RouteContext<()>,
        sql_id: Option<CarId>,
    ) -> worker::Result<&CarId> {
        if sql_id.is_none() {
            panic!("No SQL ID");
        }

        let car_id = match &self.id {
            Some(db_id) => db_id,
            _ => panic!("No SQL ID or doesn't match"),
        };

        if sql_id.unwrap() != *car_id {
            panic!("No SQL IDs Don't match");
        }

        let kv = ctx.env.kv("vinscrapes").expect("Couldn't get db");
        let response = kv
            .put(&self.serial_number.to_string(), &self)
            .expect("Couldn't build put options")
            .execute()
            .await;
        match response {
            Ok(_) => Ok(car_id),
            Err(_) => Err("Failed to save to kv".into()),
        }
    }

    pub async fn from_pdf(pdf_bytes: Vec<u8>) -> worker::Result<Option<Car>> {
        let pdf_text = pdf_extract::extract_text_from_mem(&pdf_bytes).expect("Couldn't parse pdf");
        let model_year = "2023";
        let model = "MODEL/OPT.CODE";
        let ext_color = "EXTERIOR COLOR";
        let int_color = "INTERIOR COLOR";
        let vin_label = "VEHICLE ID NUMBER";
        let port = "PORT OF ENTRY";
        let sold_to = "Sold To";
        let ship_to = "Ship To";
        let _model_year_index = pdf_text.find(model_year).unwrap_or(0);
        let model_index = pdf_text.find(model).unwrap_or(0);
        let ext_color_index = pdf_text.find(ext_color).unwrap_or(0);
        let int_color_index = pdf_text.find(int_color).unwrap_or(0);
        let vin_index = pdf_text.find(vin_label).unwrap_or(0);
        let port_index = pdf_text.find(port).unwrap_or(0);
        let sold_to_index = pdf_text.find(sold_to).unwrap_or(0);
        let ship_to_index = pdf_text.find(ship_to).unwrap_or(0);
        let car_description = pdf_text[..model_index].trim().to_string();

        let vin_code: Vec<&str> = pdf_text[model_index + model.len() + 1..ext_color_index]
            .split('/')
            .map(|s| s.trim())
            .collect();
        let _model_code = vin_code.get(0).unwrap_or(&"").to_string();
        let opt_code = vin_code.get(1).unwrap_or(&"").to_string();
        let ext_color_value = pdf_text[ext_color_index + ext_color.len() + 1..int_color_index]
            .trim()
            .to_string();
        let int_color_value = pdf_text[int_color_index + int_color.len() + 1..vin_index]
            .trim()
            .to_string();
        let vin_value = pdf_text[vin_index + vin_label.len() + 1..port_index]
            .trim()
            .to_string();
        let sold_to_value = pdf_text[sold_to_index + sold_to.len() + 2..ship_to_index]
            .trim()
            .to_string();
        let ship_to_value = pdf_text
            [ship_to_index + ship_to.len() + 2..ship_to_index + ship_to.len() + 2 + 5]
            .trim()
            .to_string();
        let dealer_address = sold_to_value.replace(&ship_to_value, "").trim().to_string();
        let _zip = dealer_address[dealer_address.len() - 5..].to_string();
        let car = Car {
            id: None,
            vin: Vin(vin_value.clone()),
            ext_color: ext_color_value,
            int_color: int_color_value,
            car_model: car_description,
            opt_code,
            ship_to: ship_to_value.clone(),
            sold_to: sold_to_value[..5].to_string(),
            created_date: Utc::now(),
            serial_number: Vin(vin_value).into(),
            model_year: model_year.to_string(),
            dead_until: None,
            last_attempt: Some(Utc::now().to_string()),
        };
        Ok(Some(car))
    }

    pub async fn from_vinlookup(
        serial: SerialNumber,
        ctx: &RouteContext<()>,
    ) -> Result<Option<Car>> {
        console_debug!("Looking up {} in 'vinlookup'", serial);
        let vins = get_possible_vins_from_serial(&serial);
        let bucket = ctx.bucket("pdf_bucket").unwrap();
        for vin in vins.into_iter() {
            console_debug!("trying {} in 'vinlookup'", vin);
            let pdf = bucket.get(&vin).execute().await;
            match pdf {
                Ok(None) => {
                    console_debug!("checked bucket and found nothing");
                    match vinlookup::vinlookup(&vin).await {
                        Ok(data) => {
                            if data == b"SAP API limits exceeded" {
                                return Err("limits exceeded downstream".into());
                            }
                            let stored = bucket.put(&vin, data.clone()).execute().await;
                            console_debug!("after stored {}", vin);
                            match stored {
                                Ok(_) => {
                                    let car = Car::from_pdf(data).await;
                                    match car {
                                        Ok(Some(mut car)) => {
                                            let car_id: CarId = car.to_d1(&ctx).await?;
                                            car.set_id(car_id);
                                            return Ok(Some(car));
                                        }
                                        _ => continue,
                                    }
                                }
                                Err(_) => return Err("couldn't store pdf".into()),
                            }
                        }
                        Err(_) => continue,
                    };
                }
                Ok(Some(object)) => {
                    console_debug!("found {} in bucket with size: {:?}", vin, object.size());
                    if object.size() == 54 {
                        console_debug!("found broken pdf in bucket for vin:{}", vin);
                        // VIN is broken
                        let broken_string = "BROKEN".to_string();
                        let vin = Vin(vin);
                        return Ok(Some(Car::new(
                            vin.clone(),
                            broken_string.clone(),
                            broken_string.clone(),
                            broken_string.clone(),
                            broken_string.clone(),
                            broken_string.clone(),
                            broken_string.clone(),
                            Utc::now(),
                            SerialNumber::from(vin),
                            broken_string,
                            Some(Utc::now().to_string()),
                            Some(Utc::now().to_string()),
                        )));
                    }
                    let body = object.body().expect("couldn't get body");
                    let bytes = body.bytes().await.expect("could not get bytes");
                    match Car::from_pdf(bytes).await {
                        Ok(Some(car)) => {
                            console_debug!("returning car we found {:?}", car);
                            return Ok(Some(car));
                        }
                        Err(e) => return Err(e.into()),
                        Ok(None) => {
                            panic!("Parsed pdf as empty")
                        }
                    }
                }
                Err(e) => return Err(e.into()),
            }
        }
        console_debug!("returning nothing, sadly");
        Ok(None)
    }
}
