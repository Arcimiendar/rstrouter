use crate::engine::context::Context;
use crate::engine::tasks::task::{ExecutionResult, Task, TaskFactory};
use async_trait::async_trait;
use log::warn;

#[derive(Debug)]
pub struct SwitchFactory {}

#[derive(Debug)]
struct SwitchCondition {
    condition: String,
    next_task: String,
}

#[derive(Debug)]
pub struct Switch {
    conditions: Vec<SwitchCondition>,
    next_task: Option<String>,
    name: String,
}

impl TaskFactory for SwitchFactory {
    fn from_yml(&self, task_name: &str, yml: &serde_yaml_ng::Value) -> Option<Box<dyn Task>> {
        let task_body = yml.get(task_name)?;
        let switch_conditions = task_body
            .get("switch")?
            .as_sequence()?
            .iter()
            .flat_map(|f| f.as_mapping())
            .flat_map(|m| {
                let mut condition = m.get("condition")?.as_str()?.to_string();
                if condition.starts_with("${") && condition.ends_with("}") {
                    let condition_slice = &condition[2..condition.len() - 1];
                    condition = format!("${{!!({})}}", condition_slice);
                } else {
                    warn!(
                        "Condition \"{}\" is not bool and will be executed to false!",
                        condition
                    );
                }
                Some(SwitchCondition {
                    condition,
                    next_task: m.get("next")?.as_str()?.to_string(),
                })
            })
            .collect();

        let next_task = self.get_next_task(task_name, yml);

        Some(Box::new(Switch {
            conditions: switch_conditions,
            next_task: next_task,
            name: task_name.to_string(),
        }))
    }
}

impl SwitchFactory {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Task for Switch {
    async fn execute(&self, context: Context) -> ExecutionResult {
        for condition in &self.conditions {
            let executed_value = context.evaluate_expr(&condition.condition).as_bool();
            if let Some(b) = executed_value
                && b
            {
                return ExecutionResult(context, Some(condition.next_task.clone()));
            }
        }

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
            tasks::{switch::SwitchFactory, task::TaskFactory},
        },
    };
    use serde_yaml_ng::Value as YmlValue;

    #[test]
    fn factory_returns_none() {
        let factory = SwitchFactory::new();
        let value = factory.from_yml(
            "test",
            &serde_yaml_ng::from_str(
                r#"
                    test:
                      switch:
                        incorrect: condition
                "#,
            )
            .unwrap(),
        );

        assert!(value.is_none())
    }

    #[tokio::test]
    async fn test_switch_condition() {
        let factory = SwitchFactory::new();
        let task = factory
            .from_yml(
                "test",
                &serde_yaml_ng::from_str(
                    r#"
                        test:
                          switch:
                            - condition: ${some === 1}
                              next: one
                            - condition: ${some === 2}
                              next: two
                            - condition: ${incorrect}
                          next: third
                    "#,
                )
                .unwrap_or(YmlValue::Null),
            )
            .unwrap();

        let context = Context::from_request(Request::default(), "./unittest_dsl");

        context.evaluate_expr(&Context::wrap_js_code("var some = 1;"));
        let res = task.execute(context).await;
        assert_eq!(res.1.unwrap(), "one");

        res.0.evaluate_expr(&Context::wrap_js_code("var some = 2;"));
        let res = task.execute(res.0).await;
        assert_eq!(res.1.unwrap(), "two");

        res.0.evaluate_expr(&Context::wrap_js_code("var some = 3;"));
        let res = task.execute(res.0).await;
        assert_eq!(res.1.unwrap(), "third");
    }
}
