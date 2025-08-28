use std::sync::Arc;

use axum::{extract::Request, routing::MethodRouter};
use rstmytype::ApiEndpointMethod;

use crate::endpoints::parser::Endpoint;
use crate::endpoints::types::Request as LocalRequest;
use crate::engine::Engine;

pub fn get_route(chunk: Vec<&Endpoint>) -> MethodRouter {
    let mut method_router = MethodRouter::new();

    for endpoint in chunk {
        let engine = Arc::new(Engine::new(endpoint));

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

    #[test]
    fn test_get_route() {
        let endpoints_owned = vec![
            Endpoint {
                guards: vec![],
                tag: "some".to_string(),
                url_path: "/some/".to_string(),
                method: rstmytype::ApiEndpointMethod::Get,
                content: r#"
                    test: 
                      return: ok
                "#
                .to_string(),
                yml_content: serde_yaml_ng::from_str(
                    r#"
                      test:
                        return: ok
                    "#,
                )
                .unwrap(),
            },
            Endpoint {
                guards: vec![],
                tag: "some".to_string(),
                url_path: "/some/".to_string(),
                method: rstmytype::ApiEndpointMethod::Post,
                content: r#"
                    test: 
                      return: ok
                "#
                .to_string(),
                yml_content: serde_yaml_ng::from_str(
                    r#"
                      test:
                        return: ok
                    "#,
                )
                .unwrap(),
            },
        ];

        let endpoitns = endpoints_owned.iter().collect();
        let r = get_route(endpoitns);
        drop(r);
    }
}
