use serde_yaml_ng::Value as YmlValue;

use crate::engine::context::Context;

trait Task {
    fn from_yaml(value: &YmlValue) -> Self;

    fn execute(context: Context) -> Context;
}
