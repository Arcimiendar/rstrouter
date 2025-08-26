use crate::engine::context::Context;
use crate::engine::tasks::task::{ExecutionResult, Task, TaskFactory, render_obj};
use async_trait::async_trait;
use serde_yaml_ng::Value as YmlValue;

#[derive(Debug)]
pub struct MockFactory {}

#[derive(Debug)]
pub struct Mock {
    args: YmlValue,
    result: Option<String>,
    next_task: Option<String>,
    name: String,
}

impl TaskFactory for MockFactory {
    fn from_yml(&self, task_name: &str, yml: &YmlValue) -> Option<Box<dyn Task>> {
        let task_body = yml.get(task_name)?;

        if task_body.get("call")?.as_str()? != "reflect.mock" {
            return None;
        }

        let args = task_body.get("args")?.clone();
        let result = task_body
            .get("result")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let next_task = self.get_next_task(task_name, yml);

        Some(Box::new(Mock {
            args,
            result,
            next_task: next_task,
            name: task_name.to_string(),
        }))
    }
}

impl MockFactory {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Task for Mock {
    async fn execute(&self, context: Context) -> ExecutionResult {
        let rendered = render_obj(&self.args, &context);

        if let Some(res) = &self.result {
            context.evaluate_expr(&Context::wrap_js_code(&format!(
                "var {} = {};",
                res,
                rendered.to_string()
            )));
        }

        ExecutionResult(context, self.next_task.clone())
    }

    fn get_name(&self) -> &str {
        &self.name
    }
}
