use std::{collections::HashMap, error::Error, str::FromStr};

use axum::{
    extract::{FromRequest, Json, Request as AxumRequest},
    http::{HeaderMap, HeaderName, HeaderValue},
};
use serde_json::Value as JsonValue;

#[derive(Default)]
pub struct Request {
    r_query: HashMap<String, String>,
    r_header: HeaderMap,
    r_body: JsonValue,
}

impl Request {
    pub async fn from_request(r: AxumRequest) -> Self {
        let headers = r.headers().clone();

        let query_params_str = r.uri().query().unwrap_or("");
        let query_params: HashMap<String, String> =
            serde_urlencoded::from_str(query_params_str).unwrap_or_default();

        let js_val = Json::from_request(r, &())
            .await
            .map(|js: Json<_>| js.0)
            .unwrap_or(JsonValue::Null);

        Self {
            r_query: query_params,
            r_header: headers,
            r_body: js_val,
        }
    }

    pub fn new(
        headers: HashMap<String, String>,
        body: JsonValue,
        query: HashMap<String, String>,
    ) -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            r_query: query,
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

    pub fn query(&self) -> &HashMap<String, String> {
        &self.r_query
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
            HashMap::from([("a".into(), "b".into()), ("c".into(), "d".into())]),
        )
        .unwrap();

        let headers = request.headers();
        assert_eq!(headers.get("test").unwrap(), "1234");

        let query = request.query();
        assert_eq!(query.get("a").unwrap(), "b");
        assert_eq!(query.get("c").unwrap(), "d");

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

        let query = request.query();
        assert_eq!(query.get("aa").unwrap(), "bb");
        assert_eq!(query.get("cc").unwrap(), "dd");
    }

    #[test]
    fn test_default() {
        let req = Request::default();
        assert!(req.query().is_empty());
        assert!(req.headers().is_empty());
        assert!(req.body().is_null());
    }
}
