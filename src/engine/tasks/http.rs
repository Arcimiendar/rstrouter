use core::task;
use std::collections::HashMap;

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
    Connect,
    Options,
    Trace,
    Patch,
}

#[derive(Debug)]
pub struct HttpArgs {
    method: HttpMethod,
    headers: HashMap<String, String>,
    query: HashMap<String, String>,
    body: YmlValue,
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
            "connect" => HttpMethod::Connect,
            "options" => HttpMethod::Options,
            "trace" => HttpMethod::Trace,
            "patch" => HttpMethod::Patch,
            _ => return None,
        };

        Some(method)
    }

    fn parse_http_args(&self, yml: &YmlValue, method: HttpMethod) -> Option<HttpArgs> {
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
        let args = self.parse_http_args(yml, method)?;

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

        ExecutionResult(context, self.next_task.clone())
    }

    fn get_name(&self) -> &str {
        &self.name
    }
}
