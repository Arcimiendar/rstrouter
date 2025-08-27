use std::collections::HashMap;

use async_trait::async_trait;
use serde_yaml_ng::Value as YmlValue;
use url::form_urlencoded;

use crate::endpoints::types::Request;
use crate::engine::Engine;
use crate::engine::context::Context;
use crate::engine::tasks::task::{ExecutionResult, Task, TaskFactory, render_obj};

#[derive(Debug)]
pub struct TemplateFactory {}

impl TaskFactory for TemplateFactory {
    fn from_yml(&self, task_name: &str, yml: &YmlValue) -> Option<Box<dyn Task>> {
        let next_task = self.get_next_task(task_name, yml);

        let task_root = yml.get(task_name)?;

        let template_path = task_root.get("template")?.as_str()?.to_string();

        let query = task_root
            .get("query")
            .and_then(|q| q.as_mapping())
            .iter()
            .flat_map(|q| q.iter())
            .flat_map(|(k, v)| Some((k.as_str()?.to_string(), v.as_str()?.to_string())))
            .collect();

        let headers = task_root
            .get("headers")
            .and_then(|q| q.as_mapping())
            .iter()
            .flat_map(|q| q.iter())
            .flat_map(|(k, v)| Some((k.as_str()?.to_string(), v.as_str()?.to_string())))
            .collect();

        let body = task_root
            .get("body")
            .map(|v| v.clone())
            .unwrap_or(YmlValue::Null);

        Some(Box::new(Template {
            name: task_name.to_string(),
            template_path,
            next_task,
            query,
            body,
            headers,
        }))
    }
}

impl TemplateFactory {
    pub fn new() -> Self {
        Self {}
    }
}

#[derive(Debug)]
struct Template {
    template_path: String,
    name: String,
    next_task: Option<String>,
    headers: HashMap<String, String>,
    query: HashMap<String, String>,
    body: YmlValue,
}

#[async_trait]
impl Task for Template {
    async fn execute(&self, context: Context) -> ExecutionResult {
        let evalueated_expr = context.evaluate_expr(&self.template_path);
        let rendered_path = evalueated_expr.as_str().unwrap_or(&self.template_path);
        let template = std::fs::read_to_string(rendered_path)
            .ok()
            .and_then(|s| serde_yaml_ng::from_str(&s).ok())
            .unwrap_or(YmlValue::Null);
        let internal_engine = Engine::from_template(&template);

        if let Some(request) = self.create_request(&context, rendered_path) {
            let result = internal_engine.execute(request).await;
            context.set_return_value(result.1, result.0);
        }

        ExecutionResult(context, self.next_task.clone())
    }

    fn get_name(&self) -> &str {
        return &self.name;
    }
}

impl Template {
    fn create_request(&self, context: &Context, template_path: &str) -> Option<Request> {
        let body = render_obj(&self.body, context);
        let headers = self
            .headers
            .iter()
            .map(|(k, v)| (k.to_string(), context.evaluate_expr(v)))
            .flat_map(|(k, v)| Some((k, v.as_str()?.to_string())))
            .collect();

        let query = self
            .query
            .iter()
            .map(|(k, v)| (k.to_string(), context.evaluate_expr(v)))
            .flat_map(|(k, v)| Some((k, v.as_str()?.to_string())));

        let params = form_urlencoded::Serializer::new(String::new())
            .extend_pairs(query)
            .finish();

        // todo: redo it, so that request stores url string and query params hasmap instead of Uri struct
        let uri = format!("http://internal/{}?{}", template_path, params);

        Request::new(headers, body, &uri).ok()
    }
}
