use crate::engine::context::Context;
use crate::engine::tasks::task::{ExecutionResult, Task, TaskFactory, render_obj};

use async_trait::async_trait;
use log::warn;
use serde_yaml_ng::Value as YmlValue;

#[derive(Debug)]
pub struct RetFactory {}

#[derive(Debug)]
pub struct Ret {
    return_expr: YmlValue,
    status_code: u16,
    next_task: Option<String>,
    name: String,
}

impl TaskFactory for RetFactory {
    fn from_yml(&self, task_name: &str, yml: &serde_yaml_ng::Value) -> Option<Box<dyn Task>> {
        let task_body = yml.get(task_name)?;
        let return_expr = task_body.get("return")?;

        let status_code: u16 = task_body
            .get("status")
            .and_then(|v| v.as_u64())
            .and_then(|v| v.try_into().ok())
            .unwrap_or(200);

        let next_task = self.get_next_task(task_name, yml);

        Some(Box::new(Ret {
            return_expr: return_expr.clone(),
            status_code: status_code,
            next_task: next_task,
            name: task_name.to_string(),
        }))
    }
}

impl RetFactory {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Task for Ret {
    async fn execute(&self, context: Context) -> ExecutionResult {
        let return_value = render_obj(&self.return_expr, &context);
        context.set_return_value(self.status_code, return_value);

        ExecutionResult(context, self.next_task.clone())
    }

    fn get_name(&self) -> &str {
        &self.name
    }
}
