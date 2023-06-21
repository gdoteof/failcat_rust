#![allow(clippy::too_many_arguments)]

use common::ScrapeResponse;
use models::{highest_serial, Car, CarQuery, CarRepository, DealerRepository, SerialNumber};
use reqwest_wasm::header::{HeaderMap, HeaderValue};
use scraper::vinlookup::{
    self, attempt_to_scrape_from_serial, get_possible_vins_from_serial, vinlookup,
};
use worker::*;

mod common;
mod models;
mod scraper;
mod utils;

fn log_request(req: &Request) {
    let time = Date::now().to_string();
    console_log!(
        "{} - [{}], located at: {:?}, within: {}",
        time,
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

    let response = router
        .get("/", |_, _| Response::ok("Hello from Workers!"))
        .get("/worker-version", |_, ctx| {
            let version = ctx.var("WORKERS_RS_VERSION")?.to_string();
            Response::ok(version)
        })
        .get_async("/car/:serial", |_, ctx| async move {
            let id = ctx.param("serial").unwrap();
            match Car::from_d1_serial(
                SerialNumber(id.parse::<i32>().expect("could not parse CarId")),
                &ctx.env.d1("failcat_db").unwrap(),
            )
            .await
            {
                Ok(car) => Response::from_json(&car),
                Err(e) => Response::error(format!("No Car Found?: {:?}", e), 404),
            }
        })
        .get_async("/cars", |request, ctx| async move {
            let url = request.url().unwrap();
            console_log!("url: {:?}", url);
            let query_str = url.query().unwrap_or_default();
            console_log!("query_str: {:?}", query_str);
            let car_query = serde_qs::from_str::<CarQuery>(query_str).unwrap();
            console_log!("car_query: {:?}", car_query);

            let cars = CarRepository::new(ctx.env.d1("failcat_db").unwrap())
                .get_all_paginated(car_query)
                .await?;
            Response::from_json(&cars)
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
                                .to_d1(ctx.env.d1("failcat_db")?)
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
            let next_serial_number = highest_serial(&ctx).await + 1.into();
            match attempt_to_scrape_from_serial(next_serial_number, &ctx).await {
                Ok(car_id) => Response::from_json(&car_id),
                Err(e) => Response::error(e.to_string(), 500),
            }
        })
        .get_async("/scrape_next/:n", |_, ctx| async move {
            let num: SerialNumber = ctx.param("n").unwrap().into();
            let next_serial_number = highest_serial(&ctx).await + num;
            match attempt_to_scrape_from_serial(next_serial_number, &ctx).await {
                Ok(car_id) => Response::from_json(&car_id),
                Err(e) => Response::error(e.to_string(), 500),
            }
        })
        .get_async("/scrape_below/:n", |_, ctx| async move {
            let num: SerialNumber = ctx.param("n").unwrap().into();
            let next_serial_number = Car::first_unknown_serial_below(&ctx, num).await?;
            if let Some(next_serial_number) = next_serial_number {
                match attempt_to_scrape_from_serial(next_serial_number, &ctx).await {
                    Ok(Some(car_id)) => Response::from_json(&ScrapeResponse::found(next_serial_number, car_id)),
                    Ok(None) => Response::from_json(&ScrapeResponse::not_found(next_serial_number)),
                    Err(e) => Response::error(e.to_string(), 500),
                }
            } else {
                Response::error("No more cars to scrape", 404)
            }
        })
        .get_async("/scrape_above/:n", |_, ctx| async move {
            let num: SerialNumber = ctx.param("n").unwrap().into();
            let next_serial_number = Car::first_unknown_serial_above(&ctx, num).await?;
            if let Some(next_serial_number) = next_serial_number {
                match attempt_to_scrape_from_serial(next_serial_number, &ctx).await {
                    Ok(Some(car_id)) => Response::from_json(&ScrapeResponse::found(next_serial_number, car_id)),
                    Ok(None) => Response::from_json(&ScrapeResponse::not_found(next_serial_number)),
                    Err(e) => Response::error(e.to_string(), 500),
                }
            } else {
                Response::error("No more cars to scrape", 404)
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
        .get_async("/window-sticker-view/:vin", |_, ctx| async move {
            let vin = ctx.param("vin").unwrap();
            match vinlookup(vin).await {
                Ok(data) => {
                    if data == b"SAP API limits exceeded" {
                        return Response::error("limits exceeded downstream", 429);
                    }
                    Ok(Response::with_headers(
                        Response::from_bytes(data).expect("couldn't get bytes"),
                        view_pdf_headers().into(),
                    ))
                }
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
        .get_async("/dealers", |_, ctx| async move {
            let repo = DealerRepository::new(ctx.env.d1("failcat_db")?);
            let dealers = repo.get_all().await.expect("couldn't get dealers");
            Response::from_json(&dealers)
        })
        .run(req.clone()?, env)
        .await?;
    Ok(handle_cors(&req, response))
}

fn view_pdf_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("Content-Type", HeaderValue::from_static("application/pdf"));
    headers
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
    headers
}

fn handle_cors(req: &Request, res: Response) -> Response {
    let origin = req
        .headers()
        .get("Origin")
        .unwrap_or_default()
        .unwrap_or_default();
    if origin.contains("vteng.io") || origin.contains("localhost") {
        let mut headers = HeaderMap::new();
        headers.insert(
            "Access-Control-Allow-Origin",
            HeaderValue::from_str(&origin).unwrap(),
        );
        headers.insert(
            "Access-Control-Allow-Methods",
            HeaderValue::from_static("GET, POST, OPTIONS"),
        );
        headers.insert(
            "Access-Control-Allow-Headers",
            HeaderValue::from_static("Content-Type"),
        );
        res.with_headers(headers.into())
    } else {
        res
    }
}
