use std::collections::HashMap;

use axum::extract::{FromRequest, Json, Request as AxumRequest};
use rquickjs::{Ctx as JsCtx, IntoJs, Result as JsResult, Value as JsValue};
use serde::Serialize;
use serde_json::Value as JsonValue;

#[derive(Default, Serialize)]
pub struct Request {
    params: HashMap<String, String>,
    headers: HashMap<String, String>,
    body: JsonValue,
}

impl<'js> IntoJs<'js> for Request {
    fn into_js(self, ctx: &JsCtx<'js>) -> JsResult<JsValue<'js>> {
        // TODO implement path parsing?
        rquickjs_serde::to_value(ctx.clone(), self).map_err(|e| rquickjs::Error::IntoJs {
            from: "Request",
            to: "Value",
            message: Some(format!("cannot init incoming object from request: {}", e)),
        })
    }
}

impl Request {
    pub async fn from_request(r: AxumRequest) -> Self {
        let headers = r
            .headers()
            .iter()
            .map(|(k, v)| {
                (
                    k.as_str().to_string(),
                    v.to_str().unwrap_or("default").to_string(),
                )
            })
            .collect();

        let query_params_str = r.uri().query().unwrap_or("");
        let query_params = serde_urlencoded::from_str(query_params_str).unwrap_or_default();

        let js_val = Json::from_request(r, &())
            .await
            .map(|js: Json<_>| js.0)
            .unwrap_or(JsonValue::Null);

        Self {
            params: query_params,
            headers: headers,
            body: js_val,
        }
    }

    pub fn new(
        headers: HashMap<String, String>,
        body: JsonValue,
        query: HashMap<String, String>,
    ) -> Self {
        Self {
            params: query,
            headers: headers,
            body: body,
        }
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
        );

        let headers = request.headers;
        assert_eq!(headers.get("test").unwrap(), "1234");

        let query = request.params;
        assert_eq!(query.get("a").unwrap(), "b");
        assert_eq!(query.get("c").unwrap(), "d");

        let body = request.body;
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

        let body = request.body;
        assert_eq!(body, json!({"test": "test"}));

        let headers = request.headers;
        assert_eq!(headers.get("some_h").unwrap(), "hh");

        let query = request.params;
        assert_eq!(query.get("aa").unwrap(), "bb");
        assert_eq!(query.get("cc").unwrap(), "dd");
    }

    #[test]
    fn test_default() {
        let req = Request::default();
        assert!(req.params.is_empty());
        assert!(req.headers.is_empty());
        assert!(req.body.is_null());
    }
}
