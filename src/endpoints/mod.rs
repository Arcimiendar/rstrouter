use axum::Router;
use itertools::Itertools;
use log::info;
use rstmytype::build_open_api;
use utoipa_swagger_ui::SwaggerUi;

use crate::endpoints::parser::{Endpoint, EndpointsCollection};
use crate::endpoints::route::get_route;

pub mod parser;
mod route;

pub fn load_swagger(mut app: Router, collection: &EndpointsCollection) -> Router {
    app = app.merge(SwaggerUi::new("/docs").url("/docs/openapi.json", build_open_api(collection)));

    app
}

pub fn load_dsl_endpoints(args: &crate::args::types::Args, mut app: Router) -> Router {
    let collection = EndpointsCollection::parse_from_dir(&args.dsl_path);

    info!("Loaded next endpoints collection: {}", collection);

    let flatten_endpoints = collection.endpoints.iter().chunk_by(|e| &e.url_path);

    for (key, chunk_iter) in &flatten_endpoints {
        let chunk: Vec<&Endpoint> = chunk_iter.collect();
        app = app.route(&key, get_route(chunk));
    }

    app = load_swagger(app, &collection);

    app
}
