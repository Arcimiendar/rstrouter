use crate::engine::context::Context;
use crate::engine::tasks::task::{ExecutionResult, Task, TaskFactory, render_obj};

use async_trait::async_trait;
use serde_yaml_ng::Value as YmlValue;

#[derive(Debug)]
pub struct RetFactory {}

#[derive(Debug)]
pub struct Ret {
    return_expr: YmlValue,
    status_code: u16,
    next_task: Option<String>,
    name: String,
}

impl TaskFactory for RetFactory {
    fn from_yml(&self, task_name: &str, yml: &serde_yaml_ng::Value) -> Option<Box<dyn Task>> {
        let task_body = yml.get(task_name)?;
        let return_expr = task_body.get("return")?.clone();

        let status_code: u16 = task_body
            .get("status")
            .and_then(|v| v.as_u64())
            .and_then(|v| v.try_into().ok())
            .unwrap_or(200);

        let next_task = self.get_next_task(task_name, yml);

        Some(Box::new(Ret {
            return_expr,
            status_code,
            next_task,
            name: task_name.to_string(),
        }))
    }
}

impl RetFactory {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Task for Ret {
    async fn execute(&self, mut context: Context) -> ExecutionResult {
        let return_value = render_obj(&self.return_expr, &context).await;
        context.set_return_value(self.status_code, return_value);

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
            tasks::{ret::RetFactory, task::TaskFactory},
        },
    };

    #[test]
    fn factory_returns_none() {
        let factory = RetFactory::new();
        let value = factory.from_yml(
            "test",
            &serde_yaml_ng::from_str(
                r#"
                    test:
                      assign:
                        a: b
                "#,
            )
            .unwrap(),
        );
        assert!(value.is_none());
    }

    #[test]
    fn factory_returns_task() {
        let factory = RetFactory::new();
        let value = factory.from_yml(
            "test",
            &serde_yaml_ng::from_str(
                r#"
                    test:
                      return: ok
                "#,
            )
            .unwrap(),
        );
        assert!(value.is_some());
    }

    #[tokio::test]
    async fn test_default_return_status_code() {
        let factory = RetFactory::new();
        let task = factory
            .from_yml(
                "test",
                &serde_yaml_ng::from_str(
                    r#"
                        test:
                          return: ok
                        
                        some_next_task: 
                          return: ok
                    "#,
                )
                .unwrap(),
            )
            .unwrap();

        let context = Context::from_request(Request::default(), "./unittest_dsl").await;

        let res = task.execute(context).await;
        let res_v = res.0.get_return_value();
        assert_eq!(res_v.status, 200);
    }

    #[tokio::test]
    async fn test_custom_return_status_code() {
        let factory = RetFactory::new();
        let task = factory
            .from_yml(
                "test",
                &serde_yaml_ng::from_str(
                    r#"
                        test:
                          return: ok
                          status: 201
                    "#,
                )
                .unwrap(),
            )
            .unwrap();

        let context = Context::from_request(Request::default(), "./unittest_dsl").await;

        let res = task.execute(context).await;
        let res_v = res.0.get_return_value();
        assert_eq!(res_v.status, 201);
    }

    #[tokio::test]
    async fn test_return_value() {
        let factory = RetFactory::new();
        let task = factory
            .from_yml(
                "test",
                &serde_yaml_ng::from_str(
                    r#"
                        test:
                          return: ${some}
                    "#,
                )
                .unwrap(),
            )
            .unwrap();

        let context = Context::from_request(Request::default(), "./unittest_dsl").await;

        context.evaluate_expr(&Context::wrap_js_code("var some = {a: '123'};")).await;

        let res = task.execute(context).await;
        let res_v = res.0.get_return_value();
        assert_eq!(
            res_v
                .json
                .as_object()
                .unwrap()
                .get("a")
                .unwrap()
                .as_str()
                .unwrap(),
            "123"
        );
    }

    #[tokio::test]
    async fn test_return_complex_value() {
        let factory = RetFactory::new();
        let task = factory
            .from_yml(
                "test",
                &serde_yaml_ng::from_str(
                    r#"
                        test:
                          return: 
                            - ${some}
                            - 2
                            - some: ${some}
                    "#,
                )
                .unwrap(),
            )
            .unwrap();

        let context = Context::from_request(Request::default(), "./unittest_dsl").await;

        context.evaluate_expr(&Context::wrap_js_code("var some = {a: '123'};")).await;

        let res = task.execute(context).await;
        let res_v = res.0.get_return_value();
        let arr = res_v.json.as_array().unwrap();
        assert_eq!(arr.len(), 3);
        for (i, el) in arr.iter().enumerate() {
            if i == 0 {
                assert_eq!(
                    el.as_object().unwrap().get("a").unwrap().as_str().unwrap(),
                    "123"
                );
            }
            if i == 1 {
                assert_eq!(el.as_i64().unwrap(), 2);
            }
            if i == 2 {
                assert_eq!(
                    el.as_object()
                        .unwrap()
                        .get("some")
                        .unwrap()
                        .get("a")
                        .unwrap()
                        .as_str()
                        .unwrap(),
                    "123"
                );
            }
            if i >= 3 {
                panic!("array had to be smaller");
            }
        }
    }
}
