use log::warn;
use reqwest::{
    RequestBuilder, Response,
    header::{HeaderMap, HeaderName, HeaderValue},
};
use serde_json::{Value as JsonValue, json};
use std::{collections::HashMap, str::FromStr};

use crate::engine::context::Context;
use crate::engine::tasks::task::{ExecutionResult, Task, TaskFactory};

use async_trait::async_trait;
use serde_yaml_ng::Value as YmlValue;

#[derive(Debug)]
pub struct HttpFactory {}

#[derive(Debug)]
pub enum HttpMethod {
    Get,
    Head,
    Post,
    Put,
    Delete,
    Patch,
}

impl HttpMethod {
    fn to_request_builder(&self, url: &str) -> RequestBuilder {
        let client = reqwest::Client::new();
        match self {
            Self::Get => client.get(url),
            Self::Post => client.post(url),
            Self::Head => client.head(url),
            Self::Delete => client.delete(url),
            Self::Patch => client.patch(url),
            Self::Put => client.put(url),
        }
    }
}

#[derive(Debug)]
pub struct HttpArgs {
    url: String,
    method: HttpMethod,
    headers: HashMap<String, String>,
    query: HashMap<String, String>,
    body: YmlValue,
}

impl HttpArgs {
    fn render_headers(&self, context: &Context) -> HeaderMap {
        self.headers
            .iter()
            .flat_map(|(k, v)| Some((HeaderName::from_str(k).ok()?, context.evaluate_expr(v))))
            .flat_map(|(k, v)| Some((k, HeaderValue::from_str(v.as_str()?).ok()?)))
            .collect()
    }

    fn render_query(&self, context: &Context) -> HashMap<String, String> {
        self.query
            .iter()
            .map(|(k, v)| (k.to_string(), context.evaluate_expr(v)))
            .flat_map(|(k, v)| Some((k, v.as_str()?.to_string())))
            .collect()
    }

    fn render_body_internal(&self, yml: &YmlValue, context: &Context) -> JsonValue {
        match yml {
            YmlValue::Bool(v) => JsonValue::Bool(v.clone()),
            YmlValue::Mapping(m) => JsonValue::Object(
                m.iter()
                    .flat_map(|(k, v)| {
                        Some((
                            k.as_str()?.to_string(),
                            self.render_body_internal(v, context),
                        ))
                    })
                    .collect(),
            ),
            YmlValue::Null => JsonValue::Null,
            YmlValue::Number(v) => serde_json::to_value(v).unwrap_or(JsonValue::Null),
            YmlValue::Sequence(v) => JsonValue::Array(
                v.iter()
                    .map(|el| self.render_body_internal(el, context))
                    .collect(),
            ),
            YmlValue::String(s) => context.evaluate_expr(s),
            YmlValue::Tagged(_) => JsonValue::Null, // not supported by JSON
        }
    }

    fn render_body(&self, context: &Context) -> JsonValue {
        self.render_body_internal(&self.body, context)
    }

    async fn render_response(&self, response: Response) -> JsonValue {
        let resp_value = response.json().await.unwrap_or(JsonValue::Null);
        // TODO: make put all request params
        json!({
            "request": {
                "url": self.url,
            },
            "response": {
                "body": resp_value
            }
        })
    }

    async fn do_request(&self, context: &Context) -> JsonValue {
        let request_result = self
            .method
            .to_request_builder(&self.url)
            .headers(self.render_headers(&context))
            .query(&self.render_query(context))
            .json(&self.render_body(context))
            .send()
            .await;

        let Ok(response) = request_result else {
            warn!("request to {} failed", &self.url);
            return JsonValue::Null;
        };

        self.render_response(response).await
    }
}

#[derive(Debug)]
pub struct Http {
    next_task: Option<String>,
    name: String,

    result: Option<String>,
    args: HttpArgs,
}

impl HttpFactory {
    pub fn new() -> Self {
        Self {}
    }

    fn parse_method(&self, yml: &YmlValue) -> Option<HttpMethod> {
        let method_str = yml.get("call")?.as_str()?;

        if !method_str.starts_with("http") {
            return None;
        }

        let method = match method_str.split('.').last()? {
            "get" => HttpMethod::Get,
            "head" => HttpMethod::Head,
            "post" => HttpMethod::Post,
            "put" => HttpMethod::Put,
            "delete" => HttpMethod::Delete,
            "patch" => HttpMethod::Patch,
            _ => return None,
        };

        Some(method)
    }

    fn parse_http_args(&self, yml: &YmlValue, method: HttpMethod) -> Option<HttpArgs> {
        let url = yml.get("url").and_then(|v| v.as_str())?.to_string();

        let headers = yml
            .get("headers")
            .iter()
            .flat_map(|v| v.as_mapping())
            .flat_map(|v| v.iter())
            .flat_map(|(k, v)| Some((k.as_str()?, v.as_str()?)))
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();

        let query = yml
            .get("query")
            .iter()
            .flat_map(|v| v.as_mapping())
            .flat_map(|v| v.iter())
            .flat_map(|(k, v)| Some((k.as_str()?, v.as_str()?)))
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();

        let body = yml.get("body").map(|y| y.clone()).unwrap_or(YmlValue::Null);

        Some(HttpArgs {
            url,
            method,
            headers,
            query,
            body,
        })
    }
}

impl TaskFactory for HttpFactory {
    fn from_yml(&self, task_name: &str, yml: &YmlValue) -> Option<Box<dyn Task>> {
        let method = self.parse_method(yml.get(task_name)?)?;
        let next_task = self.get_next_task(task_name, yml);
        let name = task_name.to_string();
        let result = yml
            .get(task_name)
            .and_then(|y| y.get("result"))
            .and_then(|y| y.as_str())
            .map(|s| s.to_string());
        let args = self.parse_http_args(yml.get(task_name)?.get("args")?, method)?;
        Some(Box::new(Http {
            next_task,
            name,
            result,
            args,
        }))
    }
}

#[async_trait]
impl Task for Http {
    async fn execute(&self, context: Context) -> ExecutionResult {
        // todo! implement it
        let response = self.args.do_request(&context).await;

        if let Some(result_name) = &self.result {
            context.evaluate_expr(&Context::wrap_js_code(&format!(
                "var {} = {};",
                result_name,
                response.to_string()
            )));
        }
        ExecutionResult(context, self.next_task.clone())
    }

    fn get_name(&self) -> &str {
        &self.name
    }
}
