use log::info;

use crate::engine::tasks::task::{Task, TaskFactory, ExecutionResult};

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

impl Task for Declaration {
    fn execute(&self, context: crate::engine::context::Context) -> ExecutionResult<'_> {
        // this is noop for now
        info!("Declaration was executed!");
        ExecutionResult(context, self.next_task.as_deref())
    }


    fn get_name(&self) -> &str {
        "declaration"
    }
}