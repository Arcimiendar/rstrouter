use serde_yaml_ng::Value as YmlValue;

use crate::engine::tasks::assign::AssignFactory;
use crate::engine::tasks::declaration::DeclarationFactory;
use crate::engine::tasks::http::HttpFactory;
use crate::engine::tasks::mock::MockFactory;
use crate::engine::tasks::ret::RetFactory;
use crate::engine::tasks::switch::SwitchFactory;
use crate::engine::tasks::task::{Task, TaskFactory};
use crate::engine::tasks::template::TemplateFactory;

mod assign;
mod declaration;
mod http;
mod mock;
mod ret;
mod switch;
pub mod task;
mod template;

pub fn produce_task(task_name: &str, global_value: &YmlValue) -> Option<Box<dyn Task>> {
    let factories: Vec<Box<dyn TaskFactory>> = vec![
        Box::new(DeclarationFactory::new()),
        Box::new(RetFactory::new()),
        Box::new(AssignFactory::new()),
        Box::new(SwitchFactory::new()),
        Box::new(HttpFactory::new()),
        Box::new(MockFactory::new()),
        Box::new(TemplateFactory::new()),
    ];

    factories
        .iter()
        .flat_map(|f| f.from_yml(task_name, global_value))
        .next() // returns first successfull parsed task
}
