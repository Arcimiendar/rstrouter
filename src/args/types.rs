use clap::Parser;

fn validate_bind_address(addr: &str) -> Result<String, String> {
    if addr.parse::<std::net::IpAddr>().is_ok() {
        Ok(addr.to_string())
    } else {
        Err(format!("Invalid bind address: {}", addr))
    }
}

fn validate_log_config(path: &str) -> Result<String, String> {
    let path_obj = std::path::Path::new(path);
    if path_obj.exists() && path_obj.is_file() {
        Ok(path.to_string())
    } else {
        Err(format!("Log configuration file does not exist: {}", path))
    }
}

fn validate_dsl_path(path: &str) -> Result<String, String> {
    let path_obj = std::path::Path::new(path);
    if path_obj.exists() && path_obj.is_dir() {
        Ok(path.to_string())
    } else {
        Err(format!("DSL path does not exist: {}", path))
    }
}

#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// The port to run the server on
    #[arg(short, long, env, default_value = "8090", value_parser = clap::value_parser!(u16).range(1..65535))]
    pub port: u16,

    /// Bind address for the server
    #[arg(short, long, env, default_value = "127.0.0.1", value_parser = validate_bind_address)]
    pub bind: String,

    /// Logging configuration file
    #[arg(short, long, env, default_value = None, value_parser = validate_log_config)]
    pub log_config: Option<String>,

    /// Path to the DSL files
    #[arg(short, long, env, default_value = "/DSL", value_parser = validate_dsl_path)]
    // #[arg(short, long, env, default_value = "./test_dsl", value_parser = validate_dsl_path)]
    pub dsl_path: String,
}

pub fn get_args() -> Args {
    Args::parse()
}

#[cfg(test)]
mod test {
    use std::env::set_var;

    use super::*;

    #[test]
    fn test_get_args() {
        unsafe {
            set_var("DSL_PATH", "./unittest_dsl");
        }
        get_args();
    }

    #[test]
    fn test_validate_bind_address() {
        assert!(validate_bind_address("0.0.0.0").is_ok());
        assert!(validate_bind_address("addr").is_err());
        assert!(validate_bind_address("localhost").is_err());
        assert!(validate_bind_address("10.123.10.8").is_ok());
        assert!(validate_bind_address("500.500.500.500").is_err());
    }

    #[test]
    fn test_validate_log_config() {
        assert!(validate_log_config("./dummy.json").is_ok());
        assert!(validate_log_config("./not_exists.json").is_err());
        assert!(validate_log_config("./unittest_dsl").is_err());
    }

    #[test]
    fn test_validate_dsl_path() {
        assert!(validate_dsl_path("./dummy.rs").is_err());
        assert!(validate_dsl_path("./not_exists.json").is_err());
        assert!(validate_dsl_path("./unittest_dsl").is_ok());
    }
}
