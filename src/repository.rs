use serde::{Deserialize, Serialize};
use worker::*;
use std::collections::HashMap;
use wasm_bindgen::JsValue; // Import JsValue from wasm_bindgen

#[derive(Debug, Deserialize, Serialize)]
pub struct QueryOptions {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
    pub filters: Option<HashMap<String, String>>,
}

pub struct Repository {
    table_name: String,
}

impl Repository {
    pub fn new(table_name: &str) -> Self {
        Self {
            table_name: table_name.to_string(),
        }
    }

    pub async fn query(
        &self,
        ctx: &RouteContext<()>,
        options: QueryOptions,
    ) -> worker::Result<Vec<HashMap<String, String>>> {
        let d1 = ctx.env.d1("failcat_db").expect("Couldn't get db");

        let mut query_builder = format!("SELECT * FROM {}", self.table_name);

        if let Some(filters) = options.filters.as_ref() {
            let filter_conditions: Vec<String> = filters
                .iter()
                .map(|(key, value)| format!("{} = ?", key))
                .collect();
            query_builder += &format!(" WHERE {}", filter_conditions.join(" AND "));
        }

        if let Some(sort_by) = options.sort_by {
            let sort_order = options.sort_order.unwrap_or_else(|| "ASC".to_string());
            query_builder += &format!(" ORDER BY {} {}", sort_by, sort_order);
        }

        if let Some(limit) = options.limit {
            query_builder += &format!(" LIMIT {}", limit);
        }

        if let Some(offset) = options.offset {
            query_builder += &format!(" OFFSET {}", offset);
        }

        let statement = d1.prepare(&query_builder);
        let query = statement.bind(
            options
                .filters
                .as_ref()
                .map(|f| f.values().map(|v| JsValue::from_str(v)).collect::<Vec<_>>())
                .unwrap_or_else(Vec::new)
                .as_slice(),
        )?;
        let result = query.all().await?;
        if result.success() {
            result.results()
        } else {
            Err("Failed to read from database".into())
        }
    }
}
