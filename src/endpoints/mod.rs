use axum::Router;
use itertools::Itertools;
use log::info;
use rstmytype::build_open_api;
use utoipa_swagger_ui::SwaggerUi;

use crate::endpoints::parser::EndpointsCollection;
use crate::endpoints::route::get_route;

pub mod parser;
mod route;
pub mod types;

pub fn load_swagger(app: Router, collection: &EndpointsCollection) -> Router {
    app.merge(SwaggerUi::new("/docs").url("/docs/openapi.json", build_open_api(collection)))
}

pub fn load_dsl_endpoints(args: &crate::args::types::Args, app: Router) -> Router {
    let collection = EndpointsCollection::parse_from_dir(&args.dsl_path);

    info!("Loaded next endpoints collection: {}", collection);

    let mut app = collection
        .endpoints
        .iter()
        .chunk_by(|e| &e.url_path)
        .into_iter()
        .fold(app, |app, (key, chunk_iter)| {
            app.route(key, get_route(chunk_iter.collect(), &args.dsl_path))
        });

    if !args.disable_swagger {
        info!("Loading swagger and building types");
        app = load_swagger(app, &collection);
    }

    app
}
