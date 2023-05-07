use bytes::Bytes;
use reqwest_wasm::header::{
    HeaderMap, HeaderValue, ACCEPT, ACCEPT_LANGUAGE, CACHE_CONTROL, DNT, ORIGIN, REFERER,
    USER_AGENT,
};
use reqwest_wasm::Client;
use worker::*;

pub(crate) async fn vinlookup(vin: &str) -> Result<Vec<u8>> {
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

    let client = Client::builder().default_headers(headers).build().expect("Could not construct client");
    let response = client.get(url).send().await.expect("Could not send request");

    if response.status().is_success() {
        let content = response.bytes().await.expect("Could not get response bytes");
        if content == Bytes::from("SAP API limits exceeded") {
            return Err(Error::from("SAP API limits exceeded"))
        }

        println!("content length: {}", content.len());
        println!("PDF saved to {}", output_path);
        return Ok(content.into())
    } else {
        eprintln!("Error: {}", response.status());
        return Err(Error::from(response.status().as_str()))
    }

}

use itertools::{iproduct, Itertools};
use phf::{phf_map, Map};

const VIN_DIGIT_POSITION_MULTIPLIER: [u32; 17] =
    [8, 7, 6, 5, 4, 3, 2, 10, 0, 9, 8, 7, 6, 5, 4, 3, 2];

static VIN_DIGIT_VALUES: Map<&'static str, u32> = phf_map!{
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

const VIN_YEAR: char = 'P';
pub fn get_possible_vins_from_serial(serial: &str) -> Vec<String> {
    let vin_starts = get_possible_vins_starts();
    iproduct!(vin_starts, VIN_DIGIT_VALUES.keys())
        .map(|(vin_start, vin_char)| {
            format!("{}{}{}G{:0>6}", vin_start, vin_char, VIN_YEAR, serial)
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
