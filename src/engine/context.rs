use boa_engine::{Context as JsContext, JsObject, JsString, JsValue, Source, property};
use log::{debug, warn};
use serde_json::{Value as JsonValue, json};
use serde_urlencoded;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::mpsc;
use std::thread;

use crate::endpoints::types::Request;

#[derive(Debug, Clone)]
pub struct ReturnValue {
    pub json: JsonValue,
    pub status: u16,
}

// unsafe impl Send for Context {}
// I'm tired fighting rust. Context will not be executed in multiple thread.
// and always be operatable inside single thread
// I have no idea how to fight borrow checker here
// compiles error explains nothing
// https://users.rust-lang.org/t/why-must-local-variables-in-async-functions-satisfy-send/58348/12
// as long as it stays inside that feature is fine.
// checked that drop is correct on the end of request.
// moved JS executor to a separate thread an leaft it hanging there

#[derive(Debug)]
pub struct LocalContext {
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
    let body = request.body();
    JsValue::from_json(body, context).unwrap_or(JsValue::Null)
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

impl LocalContext {
    pub fn from_request(request: Request) -> Self {
        let mut context = JsContext::default();

        let obj = build_incoming_from_request(request, &mut context);

        context
            .register_global_property(JsString::from("incoming"), obj, property::Attribute::all())
            .ok();
        Self {
            context: RefCell::new(context),
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

enum Command {
    EvaluateExpr(String),
    Exit,
}

#[derive(Debug)]
pub struct Context {
    pub status_code: RefCell<u16>,
    pub return_json: RefCell<JsonValue>,
    thread: Option<thread::JoinHandle<()>>,
    tx: mpsc::Sender<Command>,
    rx: mpsc::Receiver<JsonValue>,
}

impl Context {
    pub fn from_request(request: Request) -> Self {
        let (tx, rx) = mpsc::channel::<Command>();
        let (tx_b, rx_b) = mpsc::channel::<JsonValue>();

        let thread = thread::spawn(move || {
            let ctx = LocalContext::from_request(request);

            for command in rx.iter() {
                match command {
                    Command::EvaluateExpr(s) => {
                        if tx_b.send(ctx.evaluate_expr(&s)).is_err() {
                            return;
                        };
                    }
                    Command::Exit => return,
                };
            }
        });

        Self {
            status_code: RefCell::new(200),
            return_json: RefCell::new(JsonValue::Null),
            thread: Some(thread),
            tx,
            rx: rx_b,
        }
    }

    pub fn get_return_value(&self) -> ReturnValue {
        ReturnValue {
            json: self.return_json.borrow().clone(),
            status: self.status_code.borrow().clone(),
        }
    }

    pub fn evaluate_expr(&self, expr: &str) -> JsonValue {
        if self
            .tx
            .send(Command::EvaluateExpr(expr.to_string()))
            .is_err()
        {
            return JsonValue::Null;
        };
        self.rx.recv().unwrap_or(JsonValue::Null)
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        debug!("context is dropped");
        self.tx.send(Command::Exit).ok();
        if let Some(thread) = self.thread.take() {
            thread.join().ok();
        }
    }
}
