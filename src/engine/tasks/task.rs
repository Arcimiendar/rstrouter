use std::fmt::Debug;

use serde_yaml_ng::Value as YmlValue;

use crate::engine::context::Context;

pub struct ExecutionResult<'a>(pub Context, pub Option<&'a str>);

pub trait Task: Debug + Send + Sync {
    fn execute(&self, context: Context) -> ExecutionResult<'_>;

    fn get_name(&self) -> &str;
}

pub trait TaskFactory: Debug {
    fn from_yml(&self, task_name: &str, yml: &YmlValue) -> Option<Box<dyn Task>>;
}