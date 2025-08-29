use log::warn;
use rstmytype::{ApiEndpoint, ApiEndpointMethod, ApiProject};
use serde_yaml_ng::{Mapping as YmlMapping, Value as YmlValue};
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
    pub yml_content: YmlValue,
    pub merged_declaration: String
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
        return Some(&self.merged_declaration);
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

        let mut obj = Self {
            guards: guards.clone(),
            tag: tag.to_str()?.to_string(),
            method,
            yml_content: yml_content,
            url_path,
            merged_declaration: "".into(),
        };

        obj.merge_declaration();

        Some(obj)
    }

    fn merge_declaration(&mut self) {
        // make it called declaration
        let mut declaration = Self::merge_two_declarations(&self.yml_content.clone(), &YmlValue::Null);
        for guard in &self.guards {
            declaration = Self::merge_two_declarations(&declaration, &guard.yml_content);
        }
        
        self.merged_declaration = serde_yaml_ng::to_string(&declaration).unwrap_or("".into());
    }

    fn merge_two_declarations(decl_l: &YmlValue, decl_r: &YmlValue) -> YmlValue {

        let (map1, map2) = (decl_l.as_mapping(), decl_r.as_mapping());

        let m1 = map1
            .iter()
            .flat_map(|m| m.values());
        let m2 = map2
            .iter()
            .flat_map(|m| m.values());
        let combined_map = m1
            .chain(m2)
            .flat_map(|v| {
                let call = v.get("call")?.as_str()?;
                if call != "declare" {
                    return None;
                }
                Some(v)
            });
        // todo implement response 
        // todo implement path
        let mut description = "".to_string();
        let mut params: Vec<YmlValue> = Vec::new();
        let mut headers: Vec<YmlValue> = Vec::new();
        let mut body = YmlValue::Null;
        for val in combined_map {
            if let Some(descr) = val.get("description").and_then(|f| f.as_str()) {
                if description.len() > 0 {
                    description.push_str("; ");
                }
                description.push_str(descr);
            }

            if let Some(al_list) = val.get("allowlist") {
                if let Some(pm) = al_list.get("params").and_then(|p| p.as_sequence()) {
                    params.extend(pm.iter().map(|p| p.clone()));
                }
                if let Some(pm) = al_list.get("query").and_then(|p| p.as_sequence()) {
                    params.extend(pm.iter().map(|p| p.clone()));
                }
                if let Some(hd) = al_list.get("headers").and_then(|h| h.as_sequence()) {
                    headers.extend(hd.iter().map(|h| h.clone()));
                }
                if let Some(bd) = al_list.get("body") {
                    if body.is_null() && !bd.is_null(){
                        // todo implement merge body. Right now the first body will be accounted
                        body = bd.clone();
                    }
                }
            }
        }
        let declare_task = YmlValue::Mapping(YmlMapping::from_iter([
            (YmlValue::String("call".into()), YmlValue::String("declare".into())),
            (YmlValue::String("description".into()), YmlValue::String(description)),
            (YmlValue::String("allowlist".into()), YmlValue::Mapping(YmlMapping::from_iter([
                (YmlValue::String("params".into()), YmlValue::Sequence(params)),
                (YmlValue::String("headers".into()), YmlValue::Sequence(headers)),
                (YmlValue::String("body".into()), body),
            ]))),
        ]));

        YmlValue::Mapping(YmlMapping::from_iter([
            (YmlValue::String("declaration".into()), declare_task)
        ]))
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
