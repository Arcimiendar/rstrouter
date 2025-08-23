use log::info;
use async_trait::async_trait;

use crate::engine::tasks::task::{Task, TaskFactory, ExecutionResult};
use crate::engine::context::Context;

#[derive(Debug)]
pub struct DeclarationFactory {}

#[derive(Debug)]
struct Declaration {
    next_task: Option<String>
}

impl DeclarationFactory {
    pub fn new() -> Self {
        Self {}
    }
}

impl TaskFactory for DeclarationFactory {
    fn from_yml(&self, task_name: &str, yml: &serde_yaml_ng::Value) -> Option<Box<dyn Task>> {
        if task_name != "declaration" { return None; };

        let mapping = yml.as_mapping()?;

        let mut next_task = None;

        for key in mapping.keys() {
            let key = key.as_str()?;
            if key == "declaration" {
                continue;
            } 

            next_task = Some(key.to_string());
        }


        Some(Box::new(Declaration {
            next_task,
        }))
    }
}

#[async_trait]
impl Task for Declaration {
    async fn execute(&self, context: Context) -> ExecutionResult {
        // this is noop for now
        info!("Declaration was executed!");
        let Some(next_task) = &self.next_task else {
            return ExecutionResult(context, None);
        };
        ExecutionResult(context, Some(next_task.clone()))
    }


    fn get_name(&self) -> &str {
        "declaration"
    }
}