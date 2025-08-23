use axum::extract::Request;
use boa_engine::{Context as JsContext, JsObject, JsString, JsValue, Source, property};
use log::warn;
use serde_json::{Value as JsonValue, json};
use serde_urlencoded;
use std::cell::RefCell;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ReturnValue {
    pub json: JsonValue,
    pub status: u16,
}

unsafe impl Send for Context {}
// I'm tired fighting rust. Context will not be executed in multiple thread.
// and always be operatable inside single stupid thread
// I have no idea how to fight borrow checker here
// compiles error explains nothing

#[derive(Debug)]
pub struct Context {
    pub status_code: RefCell<u16>,
    pub return_json: RefCell<JsonValue>,
    pub context: RefCell<JsContext>,
}

fn build_headers(request: &Request, context: &mut JsContext) -> JsObject {
    let obj = JsObject::with_null_proto();
    for (k, v) in request.headers() {
        obj.set(
            JsString::from(k.as_str()),
            JsString::from(v.to_str().unwrap_or("default")),
            false,
            context,
        )
        .ok();
    }
    obj
}

fn build_params(request: &Request, context: &mut JsContext) -> JsObject {
    let obj = JsObject::with_null_proto();
    let query_params_str = request.uri().query().unwrap_or("");
    let query_params: HashMap<String, String> =
        serde_urlencoded::from_str(query_params_str).unwrap_or_default();
    for (k, v) in query_params {
        obj.set(JsString::from(k), JsString::from(v), false, context)
            .ok();
    }

    obj
}

fn build_body(request: &Request, context: &mut JsContext) -> JsValue {
    // TODO: implement it;
    JsValue::Null
}

fn build_incoming_from_request(request: Request, context: &mut JsContext) -> JsObject {
    let obj = JsObject::with_null_proto();
    obj.set(
        JsString::from("headers"),
        JsValue::from(build_headers(&request, context)),
        false,
        context,
    )
    .ok();

    obj.set(
        JsString::from("params"),
        JsValue::from(build_params(&request, context)),
        false,
        context,
    )
    .ok();

    obj.set(
        JsString::from("body"),
        JsValue::from(build_body(&request, context)),
        false,
        context,
    )
    .ok();

    obj
}

impl Context {
    pub fn from_request(request: Request) -> Self {
        let mut context = JsContext::default();

        let obj = build_incoming_from_request(request, &mut context);

        context
            .register_global_property(JsString::from("incoming"), obj, property::Attribute::all())
            .ok();
        Self {
            context: RefCell::new(context),
            status_code: RefCell::new(200),
            return_json: RefCell::new(JsonValue::Null),
        }
    }

    pub fn get_return_value(&self) -> ReturnValue {
        ReturnValue {
            json: self.return_json.borrow().clone(),
            status: self.status_code.borrow().clone(),
        }
    }

    fn substitute_template(&self, expr: String) -> String {
        // TODO: implememnt this
        expr
    }

    fn execute_js_signle_line(&self, expr: &str) -> JsonValue {
        let mut context = self.context.borrow_mut();
        let s = format!("JSON.stringify({})", expr);
        let sc = s.as_bytes(); // borrow check is crazy here

        let source = if expr.ends_with('!') {
            Source::from_bytes(expr[0..expr.len() - 1].as_bytes())
        } else {
            Source::from_bytes(sc)
        };
        context
            .eval(source)
            .map_err(|f| {
                warn!("Uncaught JS error: {}", f);
                f
            })
            .ok()
            .and_then(|f| {
                f.as_string()
                    .and_then(|s| s.to_std_string().ok())
                    .and_then(|v| serde_json::from_str(&v).ok())
            })
            .unwrap_or(JsonValue::Null)
    }

    pub fn evaluate_expr(&self, expr: &str) -> JsonValue {
        let mut expr_copy = expr.to_string();

        if expr_copy.contains("#[") && expr_copy.contains("#]") {
            expr_copy = self.substitute_template(expr_copy);
        }

        if expr_copy.starts_with("${") && expr_copy.ends_with("}") {
            return self.execute_js_signle_line(&expr_copy[2..expr_copy.len() - 1]);
        }

        if expr_copy.contains("${") && expr_copy.contains("}") {
            return self.execute_js_signle_line(&format!("`{}`", expr_copy));
        }

        json!(expr_copy)
    }
}
