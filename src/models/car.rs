use super::*;
use serde::{Deserialize, Serialize};

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
    serial_number: SerialNumber,
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

    pub async fn to_d1(&self, ctx: &RouteContext<()>) -> worker::Result<CarId> {
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
            self.serial_number.0.into(),
            self.model_year.clone().into(),
            self.dead_until.clone().into(),
            self.last_attempt.clone().into(),
        ])?;
        return match query.first(None).await? {
            Some(Car { id, .. }) => Ok(id.unwrap()),
            None => Err("No car found".into()),
        };
    }

    pub async fn from_kv(serial: &str, ctx: &RouteContext<()>) -> worker::Result<Option<Car>> {
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
    pub async fn to_kv(&self, ctx: &RouteContext<()>, _sql_id: CarId) -> worker::Result<()> {
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

    pub async fn from_pdf(
        pdfBytes: Vec<u8>,
        ctx: &RouteContext<()>,
    ) -> worker::Result<Option<Car>> {
        let pdf_text = pdf_extract::extract_text_from_mem(&pdfBytes).expect("Couldn't parse pdf");
        console_log!("Extracted: {:?}", pdf_text);
        let model_year = "TELLURIDE";
        let model = "MODEL/OPT.CODE";
        let ext_color = "EXTERIOR COLOR";
        let int_color = "INTERIOR COLOR";
        let vin_label = "VEHICLE ID NUMBER";
        let port = "PORT OF ENTRY";
        let sold_to = "Sold To";
        let ship_to = "Ship To";
        let model_year_index = pdf_text.find(model_year).unwrap_or(0);
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
        let model_code = vin_code.get(0).unwrap_or(&"").to_string();
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
        let zip = dealer_address[dealer_address.len() - 5..].to_string();
        let car = Car {
            id: None,
            vin: Vin(vin_value),
            ext_color: ext_color_value,
            int_color: int_color_value,
            car_model: car_description,
            opt_code,
            ship_to: ship_to_value.clone(),
            sold_to: sold_to_value,
            created_date: Utc::now().to_rfc2822(),
            serial_number: SerialNumber::from_str(&ship_to_value),
            model_year: model_year.to_string(),
            dead_until: None,
            last_attempt: None,
        };
        Ok(Some(car))
    }

    pub async fn from_vinlookup(
        serial: SerialNumber,
        ctx: &RouteContext<()>,
    ) -> Result<Option<Car>> {
        let vins = get_possible_vins_from_serial(&serial);
        let bucket = ctx.bucket("pdf_bucket").unwrap();
        for vin in vins.into_iter() {
            let pdf = bucket.get(&vin).execute().await;
            match pdf {
                Ok(None) => {
                    match vinlookup::vinlookup(&vin).await {
                        Ok(data) => {
                            if data == b"SAP API limits exceeded" {
                                return Err("limits exceeded downstream".into());
                            }
                            let stored = bucket.put(&vin, data.clone()).execute().await;
                            match stored {
                                Ok(_) => {
                                    let car = Car::from_pdf(data, &ctx).await;
                                    match car {
                                        Ok(Some(car)) => {
                                            let carId: CarId = car.to_d1(&ctx).await?;
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
                Ok(Some(data)) => {
                    let body = data.body().expect("couldn't get body");
                    let bytes = body.bytes().await.expect("could not get bytes");
                    return Err("pdf already exists".into());
                }
                Err(_) => continue,
            }
        }
        Ok(None)
    }
}