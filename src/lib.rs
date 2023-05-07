use serde_json::json;
use vinlookup::get_possible_vins_from_serial;
use worker::*;

mod utils;
mod vinlookup;

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
        .post_async("/form/:field", |mut req, ctx| async move {
            if let Some(name) = ctx.param("field") {
                let form = req.form_data().await?;
                match form.get(name) {
                    Some(FormEntry::Field(value)) => {
                        return Response::from_json(&json!({ name: value }))
                    }
                    Some(FormEntry::File(_)) => {
                        return Response::error("`field` param in form shouldn't be a File", 422);
                    }
                    None => return Response::error("Bad Request", 400),
                }
            }

            Response::error("Bad Request", 400)
        })
        .get("/worker-version", |_, ctx| {
            let version = ctx.var("WORKERS_RS_VERSION")?.to_string();
            Response::ok(version)
        })
        .get_async("/vinlookup/:vin", |_, ctx| async move {
            let vin = ctx.param("vin").unwrap();
            if !vinlookup::is_valid_vin(vin) {
                return Response::error("Invalid VIN", 400);
            }

            match vinlookup::vinlookup(vin).await {
                Ok(_) => {
                    get_possible_vins_from_serial("12345");
                    Response::ok("Success")
                }
                Err(e) => Response::error(e.to_string(), 500),
            }
        })
        .get_async("/serial/:serial", |_, ctx| async move {
            let serial = ctx.param("serial").unwrap();
            let vins = get_possible_vins_from_serial(serial);
            for vin in vins {
                let bucket = ctx.bucket("pdf_bucket").unwrap();
                let pdf = bucket.get(&vin).execute().await;
                match pdf {
                    Ok(None) => {
                        match vinlookup::vinlookup(&vin).await {
                            Ok(pdf) => {
                                if pdf == b"SAP API limits exceeded" {
                                    return Response::error("limits exceeded downstream", 429);
                                }
                                return Response::from_bytes(pdf)
                            },
                            Err(_) => continue,
                        };
                    }
                    Ok(Some(data)) => {
                        return Response::from_bytes(
                            data.body().expect("couldn't get body").bytes().await.expect("could not get bytes"),
                        );
                    }
                    Err(_) => continue,
                }
            }


            Response::from_json(&get_possible_vins_from_serial(serial))
        })
        .run(req, env)
        .await
}
