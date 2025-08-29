use boa_engine::{Context as JsContext, JsObject, JsString, JsValue, Source, property};
use log::{debug, warn};
use serde_json::{Value as JsonValue, json};
use serde_urlencoded;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Mutex, mpsc};
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
    pub status_code: RefCell<u16>,
    pub return_json: RefCell<JsonValue>,
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
        // TODO implement path parsing? 
        let mut context = JsContext::default();

        let obj = build_incoming_from_request(request, &mut context);

        context
            .register_global_property(JsString::from("incoming"), obj, property::Attribute::all())
            .ok();

        let ctx = Self {
            status_code: RefCell::new(200),
            return_json: RefCell::new(JsonValue::Null),
            context: RefCell::new(context),
        };

        // TODO: make it passable from params.
        let args = crate::args::types::get_args();
        let dsl = args.dsl_path;

        ctx.evaluate_expr(&Context::wrap_js_code(&format!(
            "var dsl = {}",
            JsonValue::String(dsl)
        )));
        ctx
    }

    pub fn get_return_value(&self) -> ReturnValue {
        ReturnValue {
            json: self.return_json.borrow().clone(),
            status: self.status_code.borrow().clone(),
        }
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
        let expr_copy = expr.to_string();

        if self.is_obj(&expr_copy) {
            return self.execute_js_signle_line(&expr_copy[2..expr_copy.len() - 1]);
        }

        if self.is_template_string(expr) {
            return self.execute_js_signle_line(&format!("`{}`", expr_copy));
        }

        json!(expr_copy)
    }

    fn is_obj(&self, expr: &str) -> bool {
        if !(expr.starts_with("${") && expr.ends_with("}")) {
            return false;
        }
        !expr[2..expr.len() - 1].contains("${")
    }

    fn is_template_string(&self, expr: &str) -> bool {
        expr.contains("${") && expr.contains("}")
    }
}

enum Command {
    EvaluateExpr(String),
    Exit,
    SetReturnValue(u16, JsonValue),
    GetReturnValue,
}

enum Reply {
    Json(JsonValue),
    RetValue(ReturnValue),
}

#[derive(Debug)]
pub struct Context {
    thread: Option<thread::JoinHandle<()>>,
    tx: mpsc::Sender<Command>,
    rx: Mutex<mpsc::Receiver<Reply>>,
}

impl Context {
    pub fn from_request(request: Request) -> Self {
        let (tx, rx) = mpsc::channel::<Command>();
        let (tx_b, rx_b) = mpsc::channel::<Reply>();

        let thread = thread::spawn(move || {
            let ctx = LocalContext::from_request(request);

            for command in rx.iter() {
                match command {
                    Command::EvaluateExpr(s) => {
                        if tx_b.send(Reply::Json(ctx.evaluate_expr(&s))).is_err() {
                            return;
                        };
                    }
                    Command::SetReturnValue(s, v) => {
                        *ctx.return_json.borrow_mut() = v;
                        *ctx.status_code.borrow_mut() = s;
                    }
                    Command::GetReturnValue => {
                        if tx_b.send(Reply::RetValue(ctx.get_return_value())).is_err() {
                            return;
                        }
                    }
                    Command::Exit => return,
                };
            }
        });

        Self {
            thread: Some(thread),
            tx,
            rx: Mutex::new(rx_b),
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
        let obj = self
            .rx
            .lock()
            .ok()
            .and_then(|v| v.recv().ok())
            .unwrap_or(Reply::Json(JsonValue::Null));

        match obj {
            Reply::RetValue(_) => {
                panic!("Something wrong with thread sync in context and localcontext");
            }
            Reply::Json(v) => v,
        }
    }

    pub fn get_return_value(&self) -> ReturnValue {
        if self.tx.send(Command::GetReturnValue).is_err() {
            panic!("Something wrong with thread sync in context and localcontext");
        };
        let obj = self
            .rx
            .lock()
            .ok()
            .and_then(|v| v.recv().ok())
            .unwrap_or(Reply::Json(JsonValue::Null));

        match obj {
            Reply::Json(_) => {
                panic!("Something wrong with thread sync in context and localcontext");
            }
            Reply::RetValue(v) => v,
        }
    }

    pub fn set_return_value(&self, status_code: u16, value: JsonValue) {
        if self
            .tx
            .send(Command::SetReturnValue(status_code, value))
            .is_err()
        {
            panic!("Something wrong with thread sync in context and localcontext");
        }
    }

    pub fn wrap_js_code(code: &str) -> String {
        format!("${{{}!}}", code)
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

#[cfg(test)]
mod test {
    use crate::{endpoints::types::Request, engine::context::Context};
    use serde_json::json;
    use std::collections::HashMap;

    #[test]
    fn test_context() {
        let headers = HashMap::from([("test".to_string(), "1234".to_string())]);
        let context = Context::from_request(
            Request::new(
                headers,
                json!({"a" : ["c"]}),
                "http://localhost:8090/test?a=b",
            )
            .unwrap(),
        );

        let res = context.evaluate_expr("1 ${incoming.params.a}");
        assert_eq!(res, "1 b");
        let res = context.evaluate_expr("${incoming.body.a}");
        assert_eq!(res, json!(["c"]));
        let res = context.evaluate_expr("do not modify");
        assert_eq!(res, "do not modify");
        let res = context.evaluate_expr("${incoming.headers.test}");
        assert_eq!(res, "1234");

        context.set_return_value(201, json!({"hello": "world"}));
        let res = context.get_return_value();
        assert_eq!(res.json, json!({"hello": "world"}));
        assert_eq!(res.status, 201);

        context.evaluate_expr(&Context::wrap_js_code("let someVar = 33;"));
        let res = context.evaluate_expr("${someVar}");
        assert_eq!(res, 33);

        let res = context.evaluate_expr("${dsl}");
        assert_eq!("./unittest_dsl", res);

        drop(context);
    }
}
