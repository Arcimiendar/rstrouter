use std::{collections::HashMap, error::Error, str::FromStr};

use axum::{
    extract::{FromRequest, Json, Request as AxumRequest},
    http::{HeaderMap, HeaderName, HeaderValue, Uri},
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

    pub fn new(
        headers: HashMap<String, String>,
        body: JsonValue,
        uri: &str,
    ) -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            r_uri: Uri::from_str(uri)?,
            r_header: headers
                .iter()
                .flat_map(|(k, v)| {
                    Some((
                        HeaderName::from_str(k).ok()?,
                        HeaderValue::from_str(v).ok()?,
                    ))
                })
                .collect(),
            r_body: body,
        })
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

#[cfg(test)]
mod test {
    use crate::endpoints::types::Request;
    use axum::{body::Body, extract::Request as AxumRequest};
    use serde_json::json;
    use std::collections::HashMap;

    #[test]
    fn test_request() {
        let request = Request::new(
            HashMap::from([("test".to_string(), "1234".to_string())]),
            json!({"obj": 1234}),
            "http://localhost:8090/test?a=b&c=d",
        )
        .unwrap();

        let headers = request.headers();
        assert_eq!(headers.get("test").unwrap(), "1234");

        let uri = request.uri();
        assert_eq!(uri.to_string(), "http://localhost:8090/test?a=b&c=d");

        let body = request.body().clone();
        assert_eq!(body, json!({"obj": 1234}));
    }

    #[tokio::test]
    async fn test_request_from_axum() {
        let req = AxumRequest::builder()
            .uri("http://localhost:8090/test?aa=bb&cc=dd")
            .header("some_h", "hh")
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"test": "test"}).to_string()))
            .unwrap();

        let request = Request::from_request(req).await;

        let body = request.body().clone();
        assert_eq!(body, json!({"test": "test"}));

        let headers = request.headers();
        assert_eq!(headers.get("some_h").unwrap(), "hh");

        let uri = request.uri();
        assert_eq!(uri.to_string(), "http://localhost:8090/test?aa=bb&cc=dd");
    }
}
