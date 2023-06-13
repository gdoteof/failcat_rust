use chrono::Utc;
use reqwest_wasm::header::{
    HeaderMap, HeaderValue, ACCEPT, ACCEPT_LANGUAGE, CACHE_CONTROL, DNT, ORIGIN, REFERER,
    USER_AGENT,
};
use reqwest_wasm::Client;
use worker::*;

pub async fn vinlookup(vin: &str) -> Result<Vec<u8>> {
    let url = format!("https://prod.idc.kia.us/sticker/find/{vin}");
    let output_path = format!("pdfs/{vin}.pdf");

    let mut headers = HeaderMap::new();
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    headers.insert("upgrade-insecure-requests", HeaderValue::from_static("1"));
    headers.insert(USER_AGENT, HeaderValue::from_static("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/90.0.4430.93 Safari/537.36"));
    headers.insert(ACCEPT, HeaderValue::from_static("text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.9"));
    headers.insert("sec-gpc", HeaderValue::from_static("1"));
    headers.insert("sec-fetch-site", HeaderValue::from_static("none"));
    headers.insert("sec-fetch-mode", HeaderValue::from_static("navigate"));
    headers.insert("sec-fetch-user", HeaderValue::from_static("?1"));
    headers.insert("sec-fetch-dest", HeaderValue::from_static("document"));
    headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.9"));
    headers.insert(
        "kiaws-api-key",
        HeaderValue::from_static("15724294-592e-4067-93af-515da5d1a5f2"),
    );
    headers.insert(ORIGIN, HeaderValue::from_static("https://www.kia.com"));
    headers.insert(REFERER, HeaderValue::from_static("https://www.kia.com"));
    headers.insert(DNT, HeaderValue::from_static("1"));

    let client = Client::builder()
        .default_headers(headers)
        .build()
        .expect("Could not construct client");
    let response = client
        .get(url)
        .send()
        .await
        .expect("Could not send request");

    if response.status().is_success() {
        let content = response
            .bytes()
            .await
            .expect("Could not get response bytes");
        if content == *"SAP API limits exceeded" {
            return Err(Error::from("SAP API limits exceeded"));
        }

        println!("content length: {}", content.len());
        println!("PDF saved to {}", output_path);
        Ok(content.into())
    } else {
        eprintln!("Error: {}", response.status());
        return Err(Error::from(response.status().as_str()));
    }
}

use itertools::{iproduct, Itertools};
use phf::{phf_map, Map};

use crate::models::{Car, CarId, ScraperLog, SerialNumber};

const VIN_DIGIT_POSITION_MULTIPLIER: [u32; 17] =
    [8, 7, 6, 5, 4, 3, 2, 10, 0, 9, 8, 7, 6, 5, 4, 3, 2];

static VIN_DIGIT_VALUES: Map<&'static str, u32> = phf_map! {
    "A" =>  1,
    "B" =>  2,
    "C" =>  3,
    "D" =>  4,
    "E" =>  5,
    "F" =>  6,
    "G" =>  7,
    "H" =>  8,
    "J" =>  1,
    "K" =>  2,
    "L" =>  3,
    "M" =>  4,
    "N" =>  5,
    "P" =>  7,
    "R" =>  9,
    "S" =>  2,
    "T" =>  3,
    "U" =>  4,
    "V" =>  5,
    "W" =>  6,
    "X" =>  7,
    "Y" =>  8,
    "Z" =>  9,
    "1" =>  1,
    "2" =>  2,
    "3" =>  3,
    "4" =>  4,
    "5" =>  5,
    "6" =>  6,
    "7" =>  7,
    "8" =>  8,
    "9" =>  9,
    "0" =>  0,
};

static YEAR_VIN_VALUES: Map<&'static str, char> = phf_map! {
    "2023" => 'P',
    "2024" => 'R',
};

pub struct VinYear {
    pub year: u32,
    pub vin_char: char,
}

impl VinYear {
    pub fn from_serial(serial: SerialNumber) -> Self {
        let (year, vin_char) = if serial > 411975.into() {
            (2024, YEAR_VIN_VALUES["2024"])
        } else {
            (2023, YEAR_VIN_VALUES["2023"])
        };
        Self { year, vin_char }
    }
}

pub fn get_possible_vins_from_serial(serial: &SerialNumber) -> Vec<String> {
    let vin_starts = get_possible_vins_starts();
    iproduct!(vin_starts, VIN_DIGIT_VALUES.keys())
        .map(|(vin_start, vin_char)| {
            format!(
                "{}{}{}G{:0>6}",
                vin_start,
                vin_char,
                VinYear::from_serial(*serial).vin_char,
                serial
            )
        })
        .map(|vin| format!("{}{}{}", &vin[0..8], get_check_sum_char(&vin), &vin[9..]))
        .collect_vec()
        .into_iter()
        .sorted()
        .dedup()
        .collect()
}

fn get_check_sum_char(vin: &str) -> char {
    let mut check_sum_total = 0;

    if vin.len() < 17 {
        panic!("Invalid Length: {}", vin.len());
    }

    for (i, c) in vin.chars().enumerate() {
        match VIN_DIGIT_VALUES.get(c.to_string().as_str()) {
            Some(value) => check_sum_total += value * VIN_DIGIT_POSITION_MULTIPLIER[i],
            None => panic!("Illegal Character: {}", c),
        }
    }

    let remain = check_sum_total % 11;
    if remain == 10 {
        'X'
    } else {
        std::char::from_digit(remain, 10).unwrap()
    }
}

fn get_possible_vins_starts() -> Vec<String> {
    let models = vec![2, 3, 5, 6];
    let drives = vec!['D', '4'];

    iproduct!(models, drives)
        .map(|(model, drive)| format!("5XYP{}{}GC", model, drive))
        .collect()
}
pub(crate) fn is_valid_vin(vin: &str) -> bool {
    if vin.len() != 17 {
        return false;
    }

    let c = get_check_sum_char(vin);
    c == vin.chars().nth(8).unwrap()
}

pub async fn attempt_to_scrape_from_serial(
    serial: SerialNumber,
    ctx: &RouteContext<()>,
) -> Result<Option<CarId>> {
    console_debug!("Attempting to scrape from serial: {}", serial);
    let car = Car::from_kv(serial, ctx).await;
    match car {
        Ok(Some(Car { id, .. })) => Err(format!("Car already saved.: {:?}", id).into()),
        Err(e) => Err(e),
        Ok(None) => {
            console_debug!("serial not saved to kv: {}", serial);
            let car = Car::from_vinlookup(serial, ctx)
                .await
                .expect("couldn't find car");
            match car {
                Some(mut car) => {
                    console_debug!("we found a car in vinlookup: {car:?}");
                    let car_id = match car.to_d1(ctx.env.d1("failcat_db")?).await {
                        Ok(created_id) => created_id,
                        Err(e) => {
                            console_error!("We received: an error {e:?}");
                            panic!("We received: an error writing to d1");
                        }
                    };
                    console_debug!("we have {car_id:?} for {car:?}");
                    car.set_id(car_id);
                    let kv_id = car
                        .to_kv(ctx, Some(car_id))
                        .await
                        .expect("couldn't save car to database");
                    ScraperLog::new(1, Utc::now().to_string(), "serial".to_owned(), true);
                    Ok(Some(*kv_id))
                }
                None => Ok(None),
            }
        }
    }
}
