use std::sync::Arc;

use axum::{extract::Request, routing::MethodRouter};
use log::info;
use rstmytype::ApiEndpointMethod;

use crate::{endpoints::parser::Endpoint, engine::Engine};

pub fn get_route(chunk: Vec<&Endpoint>) -> MethodRouter {
    let mut method_router = MethodRouter::new();

    for endpoint in chunk {
        let engine = Arc::new(Engine::new(endpoint));

        if endpoint.method == ApiEndpointMethod::Get {
            method_router = method_router.get(|q: Request| async move {
                engine.execute(q).await
            })
        } else if endpoint.method == ApiEndpointMethod::Post {
            method_router = method_router.post(|q: Request| async move {
                engine.execute(q).await
            })
        }
    }

    method_router
}
