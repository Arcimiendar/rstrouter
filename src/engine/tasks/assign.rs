use crate::engine::context::Context;
use crate::engine::tasks::task::{ExecutionResult, Task, TaskFactory, render_obj};
use async_trait::async_trait;
use serde_yaml_ng::Value as YmlValue;

#[derive(Debug)]
pub struct AssignFactory {}

#[derive(Debug)]
pub struct Assign {
    assign_expr: YmlValue,
    next_task: Option<String>,
    name: String,
}

impl TaskFactory for AssignFactory {
    fn from_yml(&self, task_name: &str, yml: &YmlValue) -> Option<Box<dyn Task>> {
        let task_body = yml.get(task_name)?;
        let assign_exprs = task_body.get("assign")?;

        let next_task = self.get_next_task(task_name, yml);

        Some(Box::new(Assign {
            assign_expr: assign_exprs.clone(),
            next_task: next_task,
            name: task_name.to_string(),
        }))
    }
}

impl AssignFactory {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Task for Assign {
    async fn execute(&self, context: Context) -> ExecutionResult {
        let rendered = render_obj(&self.assign_expr, &context);
        rendered
            .as_object()
            .iter()
            .flat_map(|f| f.iter())
            .map(|(k, v)| (k, v.to_string()))
            .map(|(k, v)| Context::wrap_js_code(&format!("var {} = {};", k, v)))
            .for_each(|expr| {
                context.evaluate_expr(&expr);
            });

        ExecutionResult(context, self.next_task.clone())
    }

    fn get_name(&self) -> &str {
        &self.name
    }
}
