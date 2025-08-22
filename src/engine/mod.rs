use axum::extract::Request;
use serde_yaml_ng::Value as YmlValue;
use serde_json::{Value as JsonValue, json};

use crate::endpoints::parser::{Endpoint, Guard};


mod tasks;
mod context;

#[derive(Debug)]
pub struct Engine {
    guards: Vec<Guard>,
    yml_content: YmlValue
}



impl Engine {
    pub fn new(endpoint: &Endpoint) -> Self {
        Self { guards: endpoint.guards.clone(), yml_content: endpoint.yml_content.clone() }
    }

    pub fn execute(&self, request: Request) -> JsonValue {
        json!({
            "response": JsonValue::Null,
        })
    }
}