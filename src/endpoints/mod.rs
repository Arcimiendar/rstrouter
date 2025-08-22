use axum::Router;
use log::info;
use rstmytype::build_open_api;
use utoipa_swagger_ui::SwaggerUi;

use crate::endpoints::parser::EndpointsCollection;

mod parser;

pub fn load_swagger(mut app: Router, collection: &EndpointsCollection) -> Router {
    app = app.merge(SwaggerUi::new("/docs").url("/docs/openapi.json", build_open_api(collection)));

    app
}

pub fn load_dsl_endpoints(args: &crate::args::types::Args, mut app: Router) -> Router {
    let collection = EndpointsCollection::parse_from_dir(&args.dsl_path);

    info!("Loaded net endpoints collection: {}", collection);

    app = load_swagger(app, &collection);

    app
}
