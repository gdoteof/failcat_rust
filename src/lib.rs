#![allow(clippy::too_many_arguments)]
use models::{Car, highest_serial, CarId, SerialNumber};
use reqwest_wasm::header::{HeaderMap, HeaderValue};
use scraper::vinlookup::{
    self, attempt_to_scrape_from_serial, get_possible_vins_from_serial, vinlookup,
};
use worker::*;

mod models;
mod scraper;
mod utils;

fn log_request(req: &Request) {
    console_log!(
        "{} - [{}], located at: {:?}, within: {}",
        Date::now().to_string(),
        req.path(),
        req.cf().coordinates().unwrap_or_default(),
        req.cf().region().unwrap_or_else(|| "unknown region".into())
    );
}

#[event(fetch)]
pub async fn main(req: Request, env: Env, _ctx: worker::Context) -> Result<Response> {
    log_request(&req);

    utils::set_panic_hook();

    let router = Router::new();

    router
        .get("/", |_, _| Response::ok("Hello from Workers!"))
        .get("/worker-version", |_, ctx| {
            let version = ctx.var("WORKERS_RS_VERSION")?.to_string();
            Response::ok(version)
        })
        .get_async("/car/:id", |_, ctx| async move {
            let id = ctx.param("id").unwrap();
            match Car::from_d1(
                CarId(id.parse::<i32>().expect("could not parse CarId")),
                &ctx,
            )
            .await
            {
                Ok(car) => Response::from_json(&car),
                Err(e) => Response::error(format!("No Car Found?: {:?}", e), 404),
            }
        })
        .get_async("/vinlookup/:vin", |_, ctx| async move {
            let vin = ctx.param("vin").unwrap();
            if !vinlookup::is_valid_vin(vin) {
                return Response::error("Invalid VIN", 400);
            }

            match vinlookup::vinlookup(vin).await {
                Ok(_) => Response::ok("Success"),
                Err(e) => Response::error(e.to_string(), 500),
            }
        })
        .get_async("/serial/:serial", |_, ctx| async move {
            let serial = SerialNumber(
                ctx.param("serial")
                    .unwrap()
                    .parse::<i32>()
                    .expect("could not parse serial"),
            );
            let vins = get_possible_vins_from_serial(&serial);
            let bucket = ctx.bucket("pdf_bucket").unwrap();
            for vin in vins {
                let pdf = bucket.get(&vin).execute().await;
                match pdf {
                    Ok(None) => {
                        match vinlookup::vinlookup(&vin).await {
                            Ok(data) => {
                                if data == b"SAP API limits exceeded" {
                                    return Response::error("limits exceeded downstream", 429);
                                }
                                let stored = bucket.put(&vin, data.clone()).execute().await;
                                match stored {
                                    Ok(_) => println!("stored pdf"),
                                    Err(_) => continue,
                                }
                                return Ok(Response::with_headers(
                                    Response::from_bytes(data).expect("couldn't get bytes"),
                                    file_pdf_headers(&vin).into(),
                                ));
                            }
                            Err(_) => continue,
                        };
                    }
                    Ok(Some(data)) => {
                        let body = data.body().expect("couldn't get body");
                        let bytes = body.bytes().await.expect("could not get bytes");
                        return Ok(Response::with_headers(
                            Response::from_bytes(bytes).expect("could not get bytes"),
                            file_pdf_headers(&vin).into(),
                        ));
                    }
                    Err(_) => continue,
                }
            }

            Response::from_json(&get_possible_vins_from_serial(&serial))
        })
        .post_async("/serial/:serial", |_, ctx| async move {
            let serial = ctx.param("serial").unwrap();
            let car = Car::from_kv(SerialNumber::from(serial), &ctx).await;
            match car {
                Ok(Some(car)) => Response::error(format!("Car already saved.: {:?}", car), 409),
                Err(e) => Response::error(format!("No Car Found?: {:?}", e), 404),
                Ok(None) => {
                    let car = Car::from_vinlookup(serial.into(), &ctx)
                        .await
                        .expect("couldn't find car");
                    match car {
                        Some(car) => {
                            let car_id = car
                                .to_d1(&ctx)
                                .await
                                .expect("couldn't save car to database");
                            let car = car
                                .to_kv(&ctx, Some(car_id))
                                .await
                                .expect("couldn't save car to database");
                            Response::from_json(&car)
                        }
                        None => Response::error("No Car Found", 404),
                    }
                }
            }
        })
        .get_async("/scrape_next", |_, ctx| async move {
            let next_serial_number = highest_serial(&ctx).await + 1;
            match attempt_to_scrape_from_serial(next_serial_number, &ctx).await {
                Ok(car_id) => Response::from_json(&car_id),
                Err(e) => Response::error(e.to_string(), 500),
            }
        })
        .get_async("/scrape_next/:n", |_, ctx| async move {
            let num : i8 = ctx.param("n").unwrap().parse().unwrap();
            let next_serial_number = highest_serial(&ctx).await + num.into();
            match attempt_to_scrape_from_serial(next_serial_number, &ctx).await {
                Ok(car_id) => Response::from_json(&car_id),
                Err(e) => Response::error(e.to_string(), 500),
            }
        })
        .get_async("/scrape/:serial_number", |_, ctx| async move {
            let serial_number = ctx
                .param("serial_number")
                .unwrap()
                .parse::<i32>()
                .expect("couldn't parse serial number");
            match attempt_to_scrape_from_serial(SerialNumber(serial_number), &ctx).await {
                Ok(car_id) => Response::from_json(&car_id),
                Err(e) => Response::error(e.to_string(), 500),
            }
        })
        .get_async("/window-sticker/:vin", |_, ctx| async move {
            let vin = ctx.param("vin").unwrap();
            match vinlookup(vin).await {
                Ok(data) => {
                    if data == b"SAP API limits exceeded" {
                        return Response::error("limits exceeded downstream", 429);
                    }
                    Ok(Response::with_headers(
                        Response::from_bytes(data).expect("couldn't get bytes"),
                        file_pdf_headers(vin).into(),
                    ))
                }
                Err(e) => Response::error(e.to_string(), 500),
            }
        })
        .run(req, env)
        .await
}

fn file_pdf_headers(vin: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("Content-Type", HeaderValue::from_static("application/pdf"));
    headers.insert(
        "Content-Disposition",
        HeaderValue::from_str(
            format!("attachment; filename=\"window-sticker-{vin}.pdf\"").as_ref(),
        )
        .expect("couldn't set header"),
    );
    headers.insert(
        "Access-Control-Allow-Origin",
        HeaderValue::from_static("https://failcat.vteng.io"),
    );
    headers
}
