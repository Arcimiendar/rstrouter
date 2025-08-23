use axum::extract::Request;
use axum::{http::StatusCode, response::IntoResponse};
use serde_json::{Value as JsonValue, json};
use serde_yaml_ng::Value as YmlValue;

use crate::endpoints::parser::Endpoint;
use crate::engine::context::Context;
use crate::engine::tasks::produce_task;
use crate::engine::tasks::task::Task;

mod context;
mod tasks;

#[derive(Debug)]
struct TaskTree {
    tasks: Vec<Box<dyn Task>>,
}

impl TaskTree {
    fn from_yml(yml: &YmlValue) -> Self {
        let Some(mapping) = yml.as_mapping() else {
            return Self { tasks: vec![] };
        };

        let tasks: Vec<Box<dyn Task>> = mapping
            .keys()
            .flat_map(|k| Some(k.as_str()?))
            .flat_map(|k| produce_task(k, yml))
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
    pub fn new(endpoint: &Endpoint) -> Self {
        Self {
            guards: endpoint
                .guards
                .iter()
                .map(|g| TaskTree::from_yml(&g.yml_content))
                .collect(),
            tree: TaskTree::from_yml(&endpoint.yml_content),
        }
    }

    pub async fn execute(&self, request: Request) -> EngineResponse {
        let mut context = Context::from_request(request);
        for guard in &self.guards {
            context = guard.walk_through(context).await;
            let return_value = context.get_return_value();
            if return_value.status < 200 && return_value.status >= 300 {
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
