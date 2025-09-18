use std::sync::Arc;

use axum::{extract::Request, routing::MethodRouter};
use rstmytype::ApiEndpointMethod;

use crate::endpoints::parser::Endpoint;
use crate::endpoints::types::Request as LocalRequest;
use crate::engine::Engine;

pub fn get_route(chunk: Vec<&Endpoint>, dsl_path: &str) -> MethodRouter {
    let mut method_router = MethodRouter::new();

    for endpoint in chunk {
        let engine = Arc::new(Engine::from_endpoint(endpoint, dsl_path));

        if endpoint.method == ApiEndpointMethod::Get {
            method_router = method_router.get(|q: Request| async move {
                engine.execute(LocalRequest::from_request(q).await).await
            })
        } else if endpoint.method == ApiEndpointMethod::Post {
            method_router = method_router.post(|q: Request| async move {
                engine.execute(LocalRequest::from_request(q).await).await
            })
        }
    }

    method_router
}

#[cfg(test)]
mod test {
    use crate::endpoints::{parser::Endpoint, route::get_route};
    use axum::{handler::Handler, http::Request};

    #[tokio::test]
    async fn test_get_route() {
        let endpoints_owned = vec![
            Endpoint {
                guards: vec![],
                tag: "some".to_string(),
                url_path: "/some/".to_string(),
                method: rstmytype::ApiEndpointMethod::Get,
                yml_content: serde_yaml_ng::from_str(
                    r#"
                      test:
                        return: ok get
                    "#,
                )
                .unwrap(),
                merged_declaration: "".into(),
            },
            Endpoint {
                guards: vec![],
                tag: "some".to_string(),
                url_path: "/some/".to_string(),
                method: rstmytype::ApiEndpointMethod::Post,
                yml_content: serde_yaml_ng::from_str(
                    r#"
                      test:
                        return: ok post
                    "#,
                )
                .unwrap(),
                merged_declaration: "".into(),
            },
        ];

        let endpoitns = endpoints_owned.iter().collect();
        let r = get_route(endpoitns, "./unittest_dsl");

        let req = Request::builder()
            .uri("/some/")
            .method("GET")
            .header("content-type", "text/plain")
            .body(axum::body::Body::from("hello world"))
            .unwrap();
        r.clone().call(req, ()).await;

        let req = Request::builder()
            .uri("/some/")
            .method("POST")
            .header("content-type", "text/plain")
            .body(axum::body::Body::from("hello world"))
            .unwrap();
        r.call(req, ()).await;
    }
}
