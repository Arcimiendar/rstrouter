use serde_yaml_ng::Value as YmlValue;

use crate::engine::tasks::declaration::DeclarationFactory;
use crate::engine::tasks::ret::RetFactory;
// use crate::engine::tasks::assign::AssignFactory;
use crate::engine::tasks::task::{Task, TaskFactory};

mod declaration;
mod ret;
pub mod task;
// mod assign;

pub fn produce_task(task_name: &str, global_value: &YmlValue) -> Option<Box<dyn Task>> {
    let factories: Vec<Box<dyn TaskFactory>> = vec![
        Box::new(DeclarationFactory::new()),
        Box::new(RetFactory::new()),
        // Box::new(AssignFactory::new()),
    ];

    factories
        .iter()
        .flat_map(|f| f.from_yml(task_name, global_value))
        .next() // returns first successfull parsed task
}
