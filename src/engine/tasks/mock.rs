use crate::engine::context::Context;
use crate::engine::tasks::task::{ExecutionResult, Task, TaskFactory, render_obj};
use async_trait::async_trait;
use serde_yaml_ng::Value as YmlValue;
use tokio::time::{Duration, sleep};

#[derive(Debug)]
pub struct MockFactory {}

#[derive(Debug)]
pub struct Mock {
    args: YmlValue,
    result: Option<String>,
    next_task: Option<String>,
    name: String,
    sleep_mcs: u64,
}

impl TaskFactory for MockFactory {
    fn from_yml(&self, task_name: &str, yml: &YmlValue) -> Option<Box<dyn Task>> {
        let task_body = yml.get(task_name)?;

        if task_body.get("call")?.as_str()? != "reflect.mock" {
            return None;
        }

        let args = task_body.get("args")?.clone();
        let result = task_body
            .get("result")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let sleep_mcs = task_body.get("sleep").and_then(|s| s.as_u64()).unwrap_or(0);

        let next_task = self.get_next_task(task_name, yml);

        Some(Box::new(Mock {
            args,
            result,
            next_task: next_task,
            name: task_name.to_string(),
            sleep_mcs,
        }))
    }
}

impl MockFactory {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Task for Mock {
    async fn execute(&self, context: Context) -> ExecutionResult {
        let rendered = render_obj(&self.args, &context).await;

        if let Some(res) = &self.result {
            context
                .evaluate_expr(&Context::wrap_js_code(&format!(
                    "var {} = {};",
                    res,
                    rendered.to_string()
                )))
                .await;
        }

        if self.sleep_mcs > 0 {
            sleep(Duration::from_millis(self.sleep_mcs)).await;
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
            tasks::{mock::MockFactory, task::TaskFactory},
        },
    };

    #[test]
    fn factory_returns_none() {
        let factory = MockFactory::new();
        let value = factory.from_yml(
            "test",
            &serde_yaml_ng::from_str(
                r#"
                    test:
                      args: 
                        url: https://google.com
                "#,
            )
            .unwrap(),
        );

        assert!(value.is_none());
    }

    #[tokio::test]
    async fn test_mock_tasks() {
        let factory = MockFactory::new();
        let value = factory.from_yml(
            "test",
            &serde_yaml_ng::from_str(
                r#"
                    test:
                      call: reflect.mock
                      sleep: 500
                      args: 
                        response:
                          body:
                           some: body
                      result: test
                "#,
            )
            .unwrap(),
        );

        assert!(value.is_some());

        let task = value.unwrap();

        let context = Context::from_request(Request::default(), "./unittest_dsl").await;

        let res = task.execute(context).await;
        let ctx = res.0;

        let v = ctx.evaluate_expr("${test}").await;
        assert_eq!(
            v.get("response")
                .unwrap()
                .get("body")
                .unwrap()
                .get("some")
                .unwrap(),
            "body"
        );
    }
}
