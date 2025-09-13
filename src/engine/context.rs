use log::{debug, warn};
use rquickjs::{
    Context as JsContext, Ctx as JsCtx, IntoJs, Result as JsResult, Runtime as JsRuntime,
    Value as JsValue,
};
use serde_json::{Value as JsonValue, json};
use std::cell::RefCell;
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

impl<'js> IntoJs<'js> for Request {
    fn into_js(self, ctx: &JsCtx<'js>) -> JsResult<JsValue<'js>> {
        // TODO implement path parsing?
        rquickjs_serde::to_value(ctx.clone(), self).map_err(|e| rquickjs::Error::IntoJs {
            from: "Request",
            to: "Value",
            message: Some(format!("cannot init incoming object from request: {}", e)),
        })
    }
}

pub struct LocalContext {
    pub context: Option<JsContext>,
    pub status_code: RefCell<u16>,
    pub return_json: RefCell<JsonValue>,
}

impl LocalContext {
    pub fn from_request(request: Request, dsl_path: &str) -> Self {
        let context = Self::get_context(request).ok();
        let ctx = Self {
            status_code: RefCell::new(200),
            return_json: RefCell::new(JsonValue::Null),
            context: context,
        };

        ctx.evaluate_expr(&Context::wrap_js_code(&format!(
            "var dsl = {}",
            JsonValue::String(dsl_path.to_string())
        )));

        ctx
    }

    fn get_context(request: Request) -> JsResult<JsContext> {
        let rt = JsRuntime::new()?;
        let context = JsContext::full(&rt)?;

        context.with(|ctx| -> JsResult<()> {
            let incoming = request.into_js(&ctx)?;
            ctx.globals().set("incoming", incoming)?;

            Ok(())
        })?;

        Ok(context)
    }

    pub fn get_return_value(&self) -> ReturnValue {
        ReturnValue {
            json: self.return_json.borrow().clone(),
            status: self.status_code.borrow().clone(),
        }
    }

    fn execute_js_signle_line(&self, expr: &str) -> JsonValue {
        let source = if expr.ends_with('!') {
            &expr[0..expr.len() - 1]
        } else {
            &expr
        };

        if let Some(context) = &self.context {
            return context.with(|ctx| -> JsonValue {
                ctx.eval(source)
                    .ok()
                    .and_then(|v: JsValue| {
                        rquickjs_serde::from_value(v).ok()
                    })
                    .unwrap_or(JsonValue::Null)
            });
        } else {
            warn!("Failed to create runtime for js. returning Null");
            return JsonValue::Null;
        }
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
    pub fn from_request(request: Request, dsl_path: &str) -> Self {
        let (tx, rx) = mpsc::channel::<Command>();
        let (tx_b, rx_b) = mpsc::channel::<Reply>();
        let dsl_path_thread_local = dsl_path.to_string();
        let thread = thread::spawn(move || {
            let ctx = LocalContext::from_request(request, &dsl_path_thread_local);

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
            debug!("thread is joined");
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
        let query = HashMap::from([("a".to_string(), "b".to_string())]);
        let context = Context::from_request(
            Request::new(headers, json!({"a" : ["c"]}), query),
            "./unittest_dsl",
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
