use serde_json::Value as JsonValue;
use axum::extract::Request;


pub struct ReturnValue {
    pub json: JsonValue,
    pub status: u16,
}

#[derive(Debug)]
pub struct Context {

}


impl Context {
    pub fn from_request(request: Request) -> Self {
        Self {}
    }

    pub fn get_return_value(&self) -> &ReturnValue {
        &ReturnValue { json: JsonValue::Null, status: 200 }
    }
}
