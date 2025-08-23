use crate::engine::context::Context;
use crate::engine::tasks::task::{ExecutionResult, Task, TaskFactory};
use async_trait::async_trait;

#[derive(Debug)]
pub struct AssignFactory {}

#[derive(Debug)]
pub struct Assign {
    return_expr: String,
    status_code: u16,
    next_task: Option<String>,
    name: String,
}

impl TaskFactory for AssignFactory {
    fn from_yml(&self, task_name: &str, yml: &serde_yaml_ng::Value) -> Option<Box<dyn Task>> {
        let task_body = yml.get(task_name)?;
        let assign_exprs = task_body.get("assign")?.as_mapping()?;

        let status_code: u16 = task_body
            .get("status")
            .and_then(|v| v.as_u64())
            .and_then(|v| v.try_into().ok())
            .unwrap_or(200);

        let next_task = self.get_next_task(task_name, yml);

        Some(Box::new(Assign {
            return_expr: return_expr.to_string(),
            status_code: status_code,
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
        let return_value = context.evaluate_expr(&self.return_expr);
        *context.return_json.borrow_mut() = return_value;
        *context.status_code.borrow_mut() = self.status_code;

        ExecutionResult(context, self.next_task.clone())
    }

    fn get_name(&self) -> &str {
        &self.name
    }
}
