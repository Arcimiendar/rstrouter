use axum::{http::StatusCode, response::IntoResponse};
use serde_json::{Value as JsonValue, json};
use serde_yaml_ng::Value as YmlValue;

use crate::endpoints::parser::Endpoint;
use crate::endpoints::types::Request;
use crate::engine::context::Context;
use crate::engine::tasks::produce_task;
use crate::engine::tasks::task::{Task, preprocess_obj};

mod context;
mod tasks;

#[derive(Debug)]
struct TaskTree {
    tasks: Vec<Box<dyn Task>>,
}

impl TaskTree {
    fn from_yml(yml: &YmlValue) -> Self {
        let preprocessed_yml = preprocess_obj(yml);
        let Some(mapping) = preprocessed_yml.as_mapping() else {
            return Self { tasks: vec![] };
        };

        let tasks: Vec<Box<dyn Task>> = mapping
            .keys()
            .flat_map(|k| Some(k.as_str()?))
            .flat_map(|k| produce_task(k, &preprocessed_yml))
            .collect();

        Self { tasks: tasks }
    }

    async fn walk_through(&self, context: Context) -> Context {
        let Some(task) = self.tasks.first() else {
            return context;
        };

        let mut res = task.execute(context).await;
        while let Some(next) = res.1 {
            let mut next_task = None;
            for task in &self.tasks {
                if task.get_name() == next {
                    next_task = Some(task);
                }
            }

            let Some(task) = next_task else {
                break;
            };
            res = task.execute(res.0).await;
        }

        return res.0;
    }
}

#[derive(Debug)]
pub struct Engine {
    guards: Vec<TaskTree>,
    tree: TaskTree,
}

pub struct EngineResponse(pub JsonValue, pub u16);

impl IntoResponse for EngineResponse {
    fn into_response(self) -> axum::response::Response {
        (
            StatusCode::from_u16(self.1).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            self.0.to_string(),
        )
            .into_response()
    }
}

impl Engine {
    pub fn from_endpoint(endpoint: &Endpoint) -> Self {
        Self {
            guards: endpoint
                .guards
                .iter()
                .map(|g| TaskTree::from_yml(&g.yml_content))
                .collect(),
            tree: TaskTree::from_yml(&endpoint.yml_content),
        }
    }

    pub fn from_template(template: &YmlValue) -> Self {
        Self {
            guards: vec![],
            tree: TaskTree::from_yml(&template),
        }
    }

    pub async fn execute(&self, request: Request) -> EngineResponse {
        let mut context = Context::from_request(request);
        for guard in &self.guards {
            context = guard.walk_through(context).await;
            let return_value = context.get_return_value();
            if return_value.status < 200 || return_value.status >= 300 {
                return EngineResponse(
                    json!({
                        "response": return_value.json
                    }),
                    return_value.status,
                );
            }
        }

        context = self.tree.walk_through(context).await;
        let return_value = context.get_return_value();

        EngineResponse(
            json!({
                "response": return_value.json,
            }),
            return_value.status,
        )
    }
}

#[cfg(test)]
mod test {
    use crate::{
        endpoints::{
            parser::{Endpoint, Guard},
            types::Request,
        },
        engine::Engine,
    };
    use axum::response::IntoResponse;
    use serde_json::{Value as JsonValue, json};
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_from_template() {
        let engine = Engine::from_template(
            &serde_yaml_ng::from_str(
                r#"
                    return:
                      return: ok
                "#,
            )
            .unwrap(),
        );

        let res = engine
            .execute(
                Request::new(
                    HashMap::new(),
                    JsonValue::Null,
                    "http://localhost:8090/test",
                )
                .unwrap(),
            )
            .await;

        assert_eq!(res.0, json!({"response": "ok"}));
    }

    #[tokio::test]
    async fn test_from_endpoint_with_guards() {
        let endpoint = Endpoint {
            guards: vec![Guard {
                yml_content: serde_yaml_ng::from_str(
                    r#"
                        condition: 
                          switch:
                            - condition: ${incoming.params.error === "error"}
                              next: error

                        test: 
                          return: ok
                          next: end

                        error:
                          return: guard return
                          status: 400
                    "#,
                )
                .unwrap(),
            }],
            tag: "some".to_string(),
            url_path: "/some/".to_string(),
            method: rstmytype::ApiEndpointMethod::Get,
            yml_content: serde_yaml_ng::from_str(
                r#"
                    test:
                      return: ok
                "#,
            )
            .unwrap(),
            merged_declaration: "".into(),
        };

        let engine = Engine::from_endpoint(&endpoint);

        let res = engine
            .execute(
                Request::new(
                    HashMap::new(),
                    JsonValue::Null,
                    "http://localhost:8090/test",
                )
                .unwrap(),
            )
            .await;

        assert_eq!(res.0, json!({"response": "ok"}));
        assert_eq!(res.1, 200);

        let res = engine
            .execute(
                Request::new(
                    HashMap::new(),
                    JsonValue::Null,
                    "http://localhost:8090/test?error=error",
                )
                .unwrap(),
            )
            .await;

        assert_eq!(res.0, json!({"response": "guard return"}));
        assert_eq!(res.1, 400);

        let resp = res.into_response();
        let status = resp.status();
        assert_eq!(status.as_u16(), 400);
    }
}
