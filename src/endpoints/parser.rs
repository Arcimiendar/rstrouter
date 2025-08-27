use log::warn;
use rstmytype::{ApiEndpoint, ApiEndpointMethod, ApiProject};
use serde_yaml_ng::{self, Value as YmlValue};
use std::ffi::OsStr;
use std::fs::{DirEntry, read_dir, read_to_string};
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Guard {
    pub yml_content: YmlValue,
}

#[derive(Debug, Clone)]
pub struct Endpoint {
    pub guards: Vec<Guard>,
    pub tag: String,
    pub url_path: String,
    pub method: ApiEndpointMethod,
    pub content: String,
    pub yml_content: YmlValue,
}

#[derive(Debug)]
pub struct EndpointsCollection {
    pub endpoints: Vec<Endpoint>,
}

impl ApiEndpoint for Endpoint {
    fn get_url_path(&self) -> &str {
        &self.url_path
    }

    fn get_endpoint_method(&self) -> &ApiEndpointMethod {
        &self.method
    }

    fn get_endpoint_tag(&self) -> &str {
        &self.tag
    }

    fn get_yml_declaration_str(&self) -> Option<&str> {
        Some(&self.content)
    }
}

impl ApiProject for EndpointsCollection {
    fn get_title(&self) -> &str {
        "rstrouter"
    }

    fn get_endpoints_iter<'a>(&'a self) -> impl Iterator<Item = &'a impl ApiEndpoint> {
        self.endpoints.iter()
    }
}

impl EndpointsCollection {
    pub fn parse_from_dir(dsl_dir: &str) -> Self {
        Self {
            endpoints: Endpoint::parse_from_dir(dsl_dir),
        }
    }
}

impl Endpoint {
    fn parse_from_dir(dsl_dir: &str) -> Vec<Self> {
        let top_level_guard: Vec<Guard> = Guard::parse_guard_from_dir(&PathBuf::from(dsl_dir))
            .into_iter()
            .collect();

        read_dir(dsl_dir)
            .ok()
            .iter_mut()
            .flat_map(|r| r.into_iter())
            .flat_map(|e| e.ok())
            .flat_map(|e| Self::parse_from_project_dir(&e, &top_level_guard))
            .collect()
    }

    fn parse_from_project_dir<'a>(dir: &DirEntry, guards: &Vec<Guard>) -> Vec<Self> {
        let guards: Vec<Guard> = guards
            .iter()
            .map(|g| g.clone())
            .chain(Guard::parse_guard_from_dir(&dir.path()).into_iter())
            .collect();

        let tag = dir.file_name();

        let iter: Vec<Self> = read_dir(dir.path())
            .ok()
            .iter_mut()
            .flat_map(|r| r.into_iter())
            .flat_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .filter(|e| !e.path().ends_with("TEMPLATES"))
            .map(|e| {
                let method = if e.path().ends_with("GET") {
                    ApiEndpointMethod::Get
                } else if e.path().ends_with("POST") {
                    ApiEndpointMethod::Post
                } else {
                    warn!(
                        "found unsupported method {} for project {}",
                        e.path().display(),
                        dir.path().display()
                    );
                    return None;
                };
                let url_path = format!("/{}", tag.to_str()?);
                Some(Self::parse_with_method(&e, &tag, url_path, method, &guards))
            })
            .flat_map(|e| e)
            .flat_map(|e| e)
            .collect();
        iter
    }

    fn parse_with_method(
        dir: &DirEntry,
        tag: &OsStr,
        url_path: String,
        method: ApiEndpointMethod,
        guards: &Vec<Guard>,
    ) -> Vec<Endpoint> {
        let guards: Vec<Guard> = guards
            .iter()
            .map(|g| g.clone())
            .chain(Guard::parse_guard_from_dir(&dir.path()).into_iter())
            .collect();

        read_dir(dir.path())
            .ok()
            .iter_mut()
            .flat_map(|r| r.into_iter())
            .flat_map(|e| e.ok())
            .flat_map(|e| {
                if e.path().is_file()
                    && !e.file_name().to_str()?.starts_with(".guard")
                    && (e.path().extension()? == "yaml" || e.path().extension()? == "yml")
                {
                    return Some(vec![Self::parse_from_file(
                        &e,
                        &tag,
                        &guards,
                        method.clone(),
                        format!("{}/{}", url_path, e.path().file_stem()?.to_str()?),
                    )?]);
                }

                Some(Self::parse_with_method(
                    &e,
                    tag,
                    format!("{}/{}", url_path, e.file_name().to_str()?),
                    method.clone(),
                    &guards,
                ))
            })
            .flat_map(|v| v)
            .collect()
    }

    fn parse_from_file(
        file: &DirEntry,
        tag: &OsStr,
        guards: &Vec<Guard>,
        method: ApiEndpointMethod,
        url_path: String,
    ) -> Option<Self> {
        let content = read_to_string(file.path()).ok()?;
        let Some(yml_content) = serde_yaml_ng::from_str(&content).ok() else {
            warn!("Endpoint {} has bad yml content", file.path().display());
            return None;
        };

        Some(Self {
            guards: guards.clone(),
            tag: tag.to_str()?.to_string(),
            method,
            content,
            yml_content: yml_content,
            url_path,
        })
    }
}

impl Guard {
    fn parse_guard_from_dir(dir: &PathBuf) -> Option<Self> {
        let mut guard_path = dir.join(".guard");
        if !guard_path.exists() || !guard_path.is_file() {
            guard_path = dir.join(".guard.yml");
        }

        if !guard_path.exists() || !guard_path.is_file() {
            guard_path = dir.join(".guard.yaml");
        }

        if !guard_path.exists() || !guard_path.is_file() {
            return None;
        }

        let content = read_to_string(&guard_path).ok()?;
        let Some(yml_content) = serde_yaml_ng::from_str(&content).ok() else {
            warn!("Guard {} has bad yml content", guard_path.display());
            return None;
        };

        Some(Self { yml_content })
    }
}

impl std::fmt::Display for EndpointsCollection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{{ endpoints: [{}] }}",
            self.endpoints
                .iter()
                .map(|e| format!("{}", e))
                .collect::<Vec<String>>()
                .join(",")
        )
    }
}

impl std::fmt::Display for Endpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{ method: {:?}, url: {} }}", self.method, self.url_path)
    }
}
