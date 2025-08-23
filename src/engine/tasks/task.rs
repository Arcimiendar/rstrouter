use std::fmt::Debug;
use async_trait::async_trait;

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
}