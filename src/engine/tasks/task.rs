use async_trait::async_trait;
use log::warn;
use serde_json::Value as JsonValue;
use std::env;
use std::fmt::Debug;

use serde_yaml_ng::Value as YmlValue;

use crate::engine::context::Context;

pub struct ExecutionResult(pub Context, pub Option<String>); // I'm tired fighting with borrow checker
// next task will not be a ptr to a task, but a name of task
// same with Option<&str> in async traits. I will clone str instead.

#[async_trait]
pub trait Task: Debug + Send + Sync {
    async fn execute(&self, context: Context) -> ExecutionResult;

    fn get_name(&self) -> &str;
}

pub trait TaskFactory: Debug {
    fn from_yml(&self, task_name: &str, yml: &YmlValue) -> Option<Box<dyn Task>>;

    fn get_next_task(&self, task_name: &str, yml: &YmlValue) -> Option<String> {
        let next_task = yml.get(task_name)?.get("next").and_then(|v| v.as_str());
        if let Some(v) = next_task {
            if v == "end" {
                return None;
            }
            return Some(v.to_string());
        }
        let mut next_task_is_next = false;
        for key in yml.as_mapping()?.keys() {
            if next_task_is_next {
                return Some(key.as_str()?.to_string());
            }
            if let Some(k) = key.as_str() {
                if k == task_name {
                    next_task_is_next = true;
                }
            }
        }
        return None;
    }
}

pub fn render_obj(yml: &YmlValue, context: &Context) -> JsonValue {
    match yml {
        YmlValue::Bool(v) => JsonValue::Bool(v.clone()),
        YmlValue::Mapping(m) => JsonValue::Object(
            m.iter()
                .flat_map(|(k, v)| Some((k.as_str()?.to_string(), render_obj(v, context))))
                .collect(),
        ),
        YmlValue::Null => JsonValue::Null,
        YmlValue::Number(v) => serde_json::to_value(v)
            .map_err(|e| {
                warn!("can't cast {} number to json: {}", v, e);
                e
            })
            .unwrap_or(JsonValue::Null),
        YmlValue::Sequence(v) => {
            JsonValue::Array(v.iter().map(|el| render_obj(el, context)).collect())
        }
        YmlValue::String(s) => context.evaluate_expr(s),
        YmlValue::Tagged(_) => {
            warn!("Tags not supported by json: {:?}", yml);
            JsonValue::Null
        } // not supported by JSON
    }
}

fn get_env_var(var_name: &str) -> String {
    match env::var(var_name) {
        Ok(s) => s,
        Err(_) => "".to_string(),
    }
}

fn fill_env_vars(value: &str) -> String {
    let mut current_str = value;
    let mut collected_str = String::with_capacity(value.len() * 10); // extra capacity to avoid realocations

    let mut cond = true;

    while cond {
        if let Some((begin, end)) = current_str.split_once("[#") {
            collected_str.push_str(begin);

            if let Some((env_var, rest)) = end.split_once("]") {
                collected_str.push_str(&get_env_var(env_var));
                current_str = rest;
            } else {
                collected_str.push_str("[#");
                collected_str.push_str(end);
                cond = false;
            }
        } else {
            collected_str.push_str(current_str);
            cond = false;
        }
    }
    collected_str.clone() // clone to remove excess mem usage
}

pub fn preprocess_obj(yml: &YmlValue) -> YmlValue {
    match yml {
        YmlValue::Mapping(m) => YmlValue::Mapping(
            m.iter()
                .map(|(k, yml)| (preprocess_obj(k), preprocess_obj(yml)))
                .collect(),
        ),
        YmlValue::Sequence(seq) => {
            YmlValue::Sequence(seq.iter().map(|yml| preprocess_obj(yml)).collect())
        }
        YmlValue::String(s) => YmlValue::String(fill_env_vars(s)),
        others => others.clone(),
    }
}
