use axum::{
    extract::{FromRequest, Json, Request as AxumRequest},
    http::{HeaderMap, Uri},
};
use serde_json::Value as JsonValue;

pub struct Request {
    r_uri: Uri,
    r_header: HeaderMap,
    r_body: JsonValue,
}

impl Request {
    pub async fn from_request(r: AxumRequest) -> Self {
        let state_val = false;
        let headers = r.headers().clone();
        let uri = r.uri().clone();
        let js_val = Json::from_request(r, &state_val)
            .await
            .map(|js: Json<_>| js.0)
            .unwrap_or(JsonValue::Null);

        Self {
            r_uri: uri,
            r_header: headers,
            r_body: js_val,
        }
    }

    pub fn headers(&self) -> &HeaderMap {
        &self.r_header
    }

    pub fn uri(&self) -> &Uri {
        &self.r_uri
    }

    pub fn body(&self) -> &JsonValue {
        &self.r_body
    }
}
