use crate::engine::context::Context;
use crate::engine::tasks::task::{ExecutionResult, Task, TaskFactory, render_obj};
use async_trait::async_trait;
use log::warn;
use serde_yaml_ng::Value as YmlValue;

#[derive(Debug)]
pub struct AssignFactory {}

#[derive(Debug)]
pub struct Assign {
    assign_expr: YmlValue,
    next_task: Option<String>,
    name: String,
}

impl TaskFactory for AssignFactory {
    fn from_yml(&self, task_name: &str, yml: &YmlValue) -> Option<Box<dyn Task>> {
        let task_body = yml.get(task_name)?;
        let assign_exprs = task_body.get("assign")?;
        if !assign_exprs.is_mapping() {
            warn!(
                "Assign task has bad syntax. Must be mapping in task {}",
                task_name
            );
            return None;
        }

        let next_task = self.get_next_task(task_name, yml);

        Some(Box::new(Assign {
            assign_expr: assign_exprs.clone(),
            next_task: next_task,
            name: task_name.to_string(),
        }))
    }
}

impl AssignFactory {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Task for Assign {
    async fn execute(&self, context: Context) -> ExecutionResult {
        let rendered = render_obj(&self.assign_expr, &context);
        rendered
            .as_object()
            .iter()
            .flat_map(|f| f.iter())
            .map(|(k, v)| (k, v.to_string()))
            .map(|(k, v)| Context::wrap_js_code(&format!("var {} = {};", k, v)))
            .for_each(|expr| {
                context.evaluate_expr(&expr);
            });

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
            tasks::{assign::AssignFactory, task::TaskFactory},
        },
    };

    #[test]
    fn test_task_is_not_parsed() {
        let factory = AssignFactory::new();

        let obj = factory.from_yml(
            "test",
            &serde_yaml_ng::from_str(
                r#"
                    test:
                      assign:
                        - 1
                        - 2
                        - 3
                "#,
            )
            .unwrap(),
        );

        assert!(obj.is_none());

        let obj = factory.from_yml(
            "test",
            &serde_yaml_ng::from_str(
                r#"
                    test:
                      return: 0
                "#,
            )
            .unwrap(),
        );

        assert!(obj.is_none());
    }

    #[tokio::test]
    async fn test_assign_task() {
        let factory = AssignFactory::new();

        let obj = factory.from_yml(
            "test",
            &serde_yaml_ng::from_str(
                r#"
                    test:
                      assign:
                        var_name: some string
                        var_rendered: ${1 + 3}
                        var_rendered2: some string ${1 + 3}
                        var_rendered3:
                            a: 5
                "#,
            )
            .unwrap(),
        );

        assert!(obj.is_some());

        let task = obj.unwrap();

        let context = Context::from_request(Request::default(), "./unittest_dsl");

        let res = task.execute(context).await;
        let ctx = res.0;
        let v = ctx.evaluate_expr("${var_name}");
        assert_eq!(v.as_str().unwrap(), "some string");
        let v = ctx.evaluate_expr("${var_rendered}");
        assert_eq!(v.as_u64().unwrap(), 4);
        let v = ctx.evaluate_expr("${var_rendered2}");
        assert_eq!(v.as_str().unwrap(), "some string 4");
        let v = ctx.evaluate_expr("${var_rendered3}");
        assert_eq!(v.get("a").unwrap().as_u64().unwrap(), 5);
    }
}
