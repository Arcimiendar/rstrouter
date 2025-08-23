use async_trait::async_trait;
use std::fmt::Debug;

use serde_yaml_ng::Value as YmlValue;

use crate::engine::context::Context;

pub struct ExecutionResult(pub Context, pub Option<String>); // I'm tired fighting with borrow checker
// next task will not be a ptr to a task, but a name of task
// same with Option<&str> in async traits. I will clone str instead.

#[async_trait]
pub trait Task: Debug + Send + Sync {
    async fn execute(&self, context: Context) -> ExecutionResult;

    fn get_name(&self) -> &str;
}

pub trait TaskFactory: Debug {
    fn from_yml(&self, task_name: &str, yml: &YmlValue) -> Option<Box<dyn Task>>;

    fn get_next_task(&self, task_name: &str, yml: &YmlValue) -> Option<String> {
        let next_task = yml.get(task_name)?.get("next").and_then(|v| v.as_str());
        if let Some(v) = next_task {
            if v == "end" {
                return None;
            }
            return Some(v.to_string());
        }
        let mut next_task_is_next = false;
        for key in yml.as_mapping()?.keys() {
            if next_task_is_next {
                return Some(key.as_str()?.to_string());
            }
            if let Some(k) = key.as_str() {
                if k == task_name {
                    next_task_is_next = true;
                }
            }
        }
        return None;
    }
}
