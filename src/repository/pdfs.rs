use std::fmt::format;

use worker::{console_debug, RouteContext};

use crate::models::Vin;

pub struct PDFRepo<'a> {
    ctx: &'a RouteContext<()>,
}

impl<'a> PDFRepo<'a> {
    pub fn new(ctx: &'a &RouteContext<()>) -> Self {
        Self { ctx }
    }

    fn key(vin: Vin) -> String {
        format!("{vin}.pdf")
    }

    pub async fn get_pdf(&self, vin: Vin) -> worker::Result<Vec<u8>> {
        let bucket = self
            .ctx
            .env
            .bucket("failcat_pdfs")
            .expect("Couldn't get bucket");
        let pdf = bucket
            .get(Self::key(vin))
            .execute()
            .await
            .expect("Couldn't get pdf")
            .expect("successfully go no pdf");
        Ok(pdf
            .body()
            .expect("couldn't get body")
            .bytes()
            .await
            .expect("couldn't get bytes"))
    }

    pub async fn delete(&self, vin: Vin) -> worker::Result<()> {
        let bucket = self
            .ctx
            .env
            .bucket("failcat_pdfs")
            .expect("Couldn't get bucket");
        let resp = bucket
            .delete(Self::key(vin))
            .await
            .expect("Couldn't delete pdf");
        console_debug!("{:?}", resp);
        Ok(())
    }
}
