use std::collections::HashMap;

use async_trait::async_trait;
use serde_yaml_ng::Value as YmlValue;

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

        let result = task_root
            .get("result")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        Some(Box::new(Template {
            name: task_name.to_string(),
            template_path,
            next_task,
            query,
            body,
            headers,
            result,
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
    result: Option<String>,
}

#[async_trait]
impl Task for Template {
    async fn execute(&self, context: Context) -> ExecutionResult {
        // TODO: make template format ruuter compatible
        let evalueated_expr = context.evaluate_expr(&format!("${{dsl}}/{}", self.template_path));
        let rendered_path = evalueated_expr.as_str().unwrap_or(&self.template_path);
        let template = std::fs::read_to_string(rendered_path)
            .ok()
            .and_then(|s| serde_yaml_ng::from_str(&s).ok())
            .unwrap_or(YmlValue::Null);
        let dsl_val = context.evaluate_expr("${dsl}");
        // will never be the value from the unwrap_or "./unittest_dsl here, because the value is always there
        let dsl_path = dsl_val.as_str().unwrap_or("./unittest_dsl");

        let internal_engine = Engine::from_template(&template, dsl_path);

        let request = self.create_request(&context);
        let result = internal_engine.execute(request).await;
        if let Some(r) = &self.result {
            context.evaluate_expr(&Context::wrap_js_code(&format!(
                "let {} = {};",
                r,
                result.0.to_string()
            )));
        }

        ExecutionResult(context, self.next_task.clone())
    }

    fn get_name(&self) -> &str {
        return &self.name;
    }
}

impl Template {
    fn create_request(&self, context: &Context) -> Request {
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
            .flat_map(|(k, v)| Some((k, v.as_str()?.to_string())))
            .collect();

        Request::new(headers, body, query)
    }
}

#[cfg(test)]
mod test {
    use crate::{
        endpoints::types::Request,
        engine::{
            context::Context,
            tasks::{task::TaskFactory, template::TemplateFactory},
        },
    };
    use serde_json::json;

    #[test]
    fn test_task_is_not_parsed() {
        let factory = TemplateFactory::new();
        let obj = factory.from_yml(
            "test",
            &serde_yaml_ng::from_str(
                r#"
                    test:
                      return: ok
                "#,
            )
            .unwrap(),
        );

        assert!(obj.is_none());
    }

    #[tokio::test]
    async fn test_template_task() {
        let factory = TemplateFactory::new();
        let obj = factory.from_yml(
            "test",
            &serde_yaml_ng::from_str(
                r#"
                    test:
                      template: test/TEMPLATES/test.yml
                      body: 
                        test: ok
                      headers:
                        test: ok
                      query:
                        test: ok
                      result: res
                "#,
            )
            .unwrap(),
        );

        assert!(obj.is_some());
        let task = obj.unwrap();

        let context = Context::from_request(Request::default(), "./unittest_dsl");

        let res = task.execute(context).await;
        let context = res.0;

        let res = context.evaluate_expr("${res}");

        assert_eq!(
            *res.get("response").unwrap(),
            json!({
                "headers": {"test": "ok"},
                "body": {"test": "ok"},
                "params": {"test": "ok"}
            })
        );
    }
}
