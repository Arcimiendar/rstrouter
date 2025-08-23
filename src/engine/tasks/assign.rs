use crate::engine::context::Context;
use crate::engine::tasks::task::{ExecutionResult, Task, TaskFactory};
use async_trait::async_trait;
use log::warn;
use serde_json::Value as JsonValue;
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

impl Assign {
    fn render_inner_strings(&self, value: &JsonValue, context: &mut Context) -> JsonValue {
        if let Some(v) = value.as_str() {
            return context.evaluate_expr(v);
        }

        if let Some(m) = value.as_object() {
            let new_m = m
                .iter()
                .map(|(k, v)| (k.to_string(), self.render_inner_strings(v, context)))
                .collect();

            return JsonValue::Object(new_m);
        }

        if let Some(a) = value.as_array() {
            let new_a: Vec<JsonValue> = a
                .iter()
                .map(|v| self.render_inner_strings(v, context))
                .collect();

            return JsonValue::Array(new_a);
        }
        // other types does not require any js evaluation
        value.clone()
    }
}

#[async_trait]
impl Task for Assign {
    async fn execute(&self, mut context: Context) -> ExecutionResult {
        if let Ok(v) = serde_json::to_value(&self.assign_expr) {
            let rendered = self.render_inner_strings(&v, &mut context);
            rendered
                .as_object()
                .iter()
                .flat_map(|f| f.iter())
                .map(|(k, v)| (k, v.to_string()))
                .map(|(k, v)| format!("${{var {} = {};!}}", k, v))
                .for_each(|expr| {
                    context.evaluate_expr(&expr);
                });
        } else {
            warn!("Incorrect assign value in {}", self.get_name());
        }

        ExecutionResult(context, self.next_task.clone())
    }

    fn get_name(&self) -> &str {
        &self.name
    }
}
