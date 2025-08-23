use async_trait::async_trait;
use log::info;

use crate::engine::context::Context;
use crate::engine::tasks::task::{ExecutionResult, Task, TaskFactory};

#[derive(Debug)]
pub struct DeclarationFactory {}

#[derive(Debug)]
struct Declaration {
    next_task: Option<String>,
}

impl DeclarationFactory {
    pub fn new() -> Self {
        Self {}
    }
}

impl TaskFactory for DeclarationFactory {
    fn from_yml(&self, task_name: &str, yml: &serde_yaml_ng::Value) -> Option<Box<dyn Task>> {
        if task_name != "declaration" {
            return None;
        };

        let next_task = self.get_next_task(task_name, yml);

        Some(Box::new(Declaration { next_task }))
    }
}

#[async_trait]
impl Task for Declaration {
    async fn execute(&self, context: Context) -> ExecutionResult {
        // this is noop for now
        info!("Declaration was executed!");
        ExecutionResult(context, self.next_task.clone())
    }

    fn get_name(&self) -> &str {
        "declaration"
    }
}
