use async_trait::async_trait;

use crate::engine::context::Context;
use crate::engine::tasks::task::{ExecutionResult, Task, TaskFactory};

#[derive(Debug)]
pub struct DeclarationFactory {}

#[derive(Debug)]
struct Declaration {
    name: String,
    next_task: Option<String>,
}

impl DeclarationFactory {
    pub fn new() -> Self {
        Self {}
    }
}

impl TaskFactory for DeclarationFactory {
    fn from_yml(&self, task_name: &str, yml: &serde_yaml_ng::Value) -> Option<Box<dyn Task>> {
        let task_root = yml.get(task_name)?;

        if task_root.get("call")?.as_str()? != "declare" {
            return None;
        }

        let next_task = self.get_next_task(task_name, yml);

        Some(Box::new(Declaration {
            name: task_name.to_string(),
            next_task,
        }))
    }
}

#[async_trait]
impl Task for Declaration {
    async fn execute(&self, context: Context) -> ExecutionResult {
        ExecutionResult(context, self.next_task.clone())
    }

    fn get_name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod test {
    use crate::{
        endpoints::types::Request,
        engine::{
            context::Context,
            tasks::{declaration::DeclarationFactory, task::TaskFactory},
        },
    };

    #[test]
    fn test_task_is_not_parsed() {
        let factory = DeclarationFactory::new();

        let obj = factory.from_yml(
            "test",
            &serde_yaml_ng::from_str(
                r#"
                    test:
                      return: ok
                "#,
            )
            .unwrap(),
        );

        assert!(obj.is_none());
    }

    #[tokio::test]
    async fn test_declaration_task() {
        let factory = DeclarationFactory::new();
        let obj = factory.from_yml(
            "test",
            &serde_yaml_ng::from_str(
                r#"
                    test:
                      call: declare
                "#,
            )
            .unwrap(),
        );

        assert!(obj.is_some());
        let task = obj.unwrap();

        let context = Context::from_request(Request::default(), "./unittest_dsl").await;

        task.execute(context).await;
    }
}
