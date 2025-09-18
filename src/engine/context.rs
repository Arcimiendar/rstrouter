use log::warn;
use rquickjs::{
    AsyncContext as AsynJsContext, Ctx as JsCtx, IntoJs, Result as JsResult, AsyncRuntime as AsyncJsRuntime,
    Value as JsValue,
};
use serde_json::{Value as JsonValue, json};
use std::sync::RwLock;

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
// Updated: made it unsafe impl Send and Sync

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

pub struct Context {
    pub context: Option<AsynJsContext>,
    pub status_code: RwLock<u16>,
    pub return_json: RwLock<JsonValue>,
}

impl Context {
    pub async fn from_request(request: Request, dsl_path: &str) -> Self {
        let context = Self::get_context(request).await.ok();
        let ctx = Self {
            status_code: RwLock::new(200),
            return_json: RwLock::new(JsonValue::Null),
            context: context,
        };

        ctx.evaluate_expr(&Context::wrap_js_code(&format!(
            "var dsl = {}",
            JsonValue::String(dsl_path.to_string())
        ))).await;
        ctx
    }

    async fn get_context(request: Request) -> JsResult<AsynJsContext> {
        let rt = AsyncJsRuntime::new()?;
        let context = AsynJsContext::full(&rt).await?;

        context.with(|ctx| -> JsResult<()> {
            let incoming = request.into_js(&ctx)?;
            ctx.globals().set("incoming", incoming)?;

            Ok(())
        }).await?;

        Ok(context)
    }

    pub fn get_return_value(&self) -> ReturnValue {
        ReturnValue {
            json: self
                .return_json
                .read()
                .map(|r| r.clone())
                .unwrap_or(JsonValue::Null),
            status: self.status_code.read().map(|r| r.clone()).unwrap_or(500),
        }
    }

    async fn execute_js_signle_line(&self, expr: &str) -> JsonValue {
        let source = if expr.ends_with('!') {
            &expr[0..expr.len() - 1]
        } else {
            &expr
        };

        if let Some(context) = &self.context {
            return context.with(|ctx| -> JsonValue {
                ctx.eval(source)
                    .ok()
                    .and_then(|v: JsValue| rquickjs_serde::from_value(v).ok())
                    .unwrap_or(JsonValue::Null)
            }).await;
        } else {
            warn!("Failed to create runtime for js. returning Null");
            return JsonValue::Null;
        }
    }

    pub async fn evaluate_expr(&self, expr: &str) -> JsonValue {
        let expr_copy = expr.to_string();

        if self.is_obj(&expr_copy) {
            return self.execute_js_signle_line(&expr_copy[2..expr_copy.len() - 1]).await;
        }

        if self.is_template_string(expr) {
            return self.execute_js_signle_line(&format!("`{}`", expr_copy)).await;
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

    pub fn set_return_value(&mut self, status_code: u16, value: JsonValue) {
        self.return_json.get_mut().map(|r| *r = value).ok();
        self.status_code.get_mut().map(|r| *r = status_code).ok();
    }

    pub fn wrap_js_code(code: &str) -> String {
        format!("${{{}!}}", code)
    }
}

#[cfg(test)]
mod test {
    use crate::{endpoints::types::Request, engine::context::Context};
    use serde_json::json;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_context() {
        let headers = HashMap::from([("test".to_string(), "1234".to_string())]);
        let query = HashMap::from([("a".to_string(), "b".to_string())]);
        let mut context = Context::from_request(
            Request::new(headers, json!({"a" : ["c"]}), query),
            "./unittest_dsl",
        ).await;

        let res = context.evaluate_expr("1 ${incoming.params.a}").await;
        assert_eq!(res, "1 b");
        let res = context.evaluate_expr("${incoming.body.a}").await;
        assert_eq!(res, json!(["c"]));
        let res = context.evaluate_expr("do not modify").await;
        assert_eq!(res, "do not modify");
        let res = context.evaluate_expr("${incoming.headers.test}").await;
        assert_eq!(res, "1234");

        context.set_return_value(201, json!({"hello": "world"}));
        let res = context.get_return_value();
        assert_eq!(res.json, json!({"hello": "world"}));
        assert_eq!(res.status, 201);

        context.evaluate_expr(&Context::wrap_js_code("let someVar = 33;")).await;
        let res = context.evaluate_expr("${someVar}").await;
        assert_eq!(res, 33);

        let res = context.evaluate_expr("${dsl}").await;
        assert_eq!("./unittest_dsl", res);

        drop(context);
    }
}
