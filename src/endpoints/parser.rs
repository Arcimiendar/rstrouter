use itertools::Itertools;
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
    pub merged_declaration: String,
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
        let mut declaration =
            Self::merge_two_declarations(&self.yml_content.clone(), &YmlValue::Null);
        for guard in &self.guards {
            declaration = Self::merge_two_declarations(&declaration, &guard.yml_content);
        }

        self.merged_declaration = serde_yaml_ng::to_string(&declaration).unwrap_or("".into());
    }

    fn merge_two_declarations(decl_l: &YmlValue, decl_r: &YmlValue) -> YmlValue {
        let (map1, map2) = (decl_l.as_mapping(), decl_r.as_mapping());

        let m1 = map1.iter().flat_map(|m| m.values());
        let m2 = map2.iter().flat_map(|m| m.values());
        let combined_map = m1.chain(m2).flat_map(|v| {
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
                    if body.is_null() && !bd.is_null() {
                        body = bd.clone();
                    } else {
                        body = Self::merge_two_types(&body, bd);
                    }
                }
            }
        }

        let headers = Self::remove_fields_duplicate(headers);
        let params = Self::remove_fields_duplicate(params);

        let declare_task = YmlValue::Mapping(YmlMapping::from_iter([
            (
                YmlValue::String("call".into()),
                YmlValue::String("declare".into()),
            ),
            (
                YmlValue::String("description".into()),
                YmlValue::String(description),
            ),
            (
                YmlValue::String("allowlist".into()),
                YmlValue::Mapping(YmlMapping::from_iter([
                    (
                        YmlValue::String("params".into()),
                        YmlValue::Sequence(params),
                    ),
                    (
                        YmlValue::String("headers".into()),
                        YmlValue::Sequence(headers),
                    ),
                    (YmlValue::String("body".into()), body),
                ])),
            ),
        ]));

        YmlValue::Mapping(YmlMapping::from_iter([(
            YmlValue::String("declaration".into()),
            declare_task,
        )]))
    }

    fn remove_fields_duplicate(to_remove: Vec<YmlValue>) -> Vec<YmlValue> {
        // todo write tests for this
        let mut new_vec = Vec::with_capacity(to_remove.len());
        let mut used_indices = Vec::with_capacity(to_remove.len());
        for (i, val_l) in to_remove.iter().enumerate() {
            if used_indices.contains(&i) {
                // was alredy merged
                continue;
            }
            let mut val_l_to_merge = val_l.clone();

            for (ii, val_r) in to_remove.iter().enumerate().skip(i + 1) {
                let name_l_opt = val_l.get("field").and_then(|f| f.as_str());
                let name_r_opt = val_r.get("field").and_then(|f| f.as_str());

                if let Some(name_l) = name_l_opt
                    && let Some(name_r) = name_r_opt
                    && name_l == name_r
                {
                    val_l_to_merge = Self::merge_two_types(&val_l_to_merge, val_r);
                    used_indices.push(ii);
                }
            }

            new_vec.push(val_l_to_merge);
        }

        new_vec.clone() // to remove more than needed mem
    }

    fn merge_two_types(b_left: &YmlValue, b_right: &YmlValue) -> YmlValue {
        // todo write tests for this

        if b_left.is_null() {
            return b_right.clone();
        }
        if b_right.is_null() {
            return b_left.clone();
        }

        if b_left.is_sequence() && b_right.is_mapping() {
            return Self::merge_seq_with_map(b_left, b_right);
        }

        if b_left.is_mapping() && b_right.is_sequence() {
            return Self::merge_seq_with_map(b_right, b_left);
        }

        if b_left.is_mapping() && b_right.is_mapping() {
            return Self::merge_mappings(b_left, b_right);
        }

        if b_left.is_sequence() && b_right.is_sequence() {
            return Self::merge_sequences(b_left, b_right);
        }

        warn!(
            "bad types format pair: {:?} and {:?}. Returning left the most appropriete",
            b_left, b_right
        );

        if b_right.is_sequence() || b_right.is_mapping() {
            return b_left.clone();
        }

        return b_left.clone();
    }

    fn merge_seq_with_map(seq: &YmlValue, obj: &YmlValue) -> YmlValue {
        let mut obj_copy = obj.clone();

        let mut merged_fields = seq.clone();

        if let Some(fields) = obj_copy.get("fields") {
            merged_fields = Self::merge_two_types(&merged_fields, fields);
        }

        if let Some(m) = obj_copy.as_mapping_mut() {
            m.insert(YmlValue::String("fields".into()), merged_fields);
        }
        return obj_copy;
    }

    fn merge_sequences(b_left: &YmlValue, b_right: &YmlValue) -> YmlValue {
        // merge obj attrs
        let default = Vec::with_capacity(0);
        let bl_seq = b_left.as_sequence().unwrap_or(&default);
        let br_seq = b_right.as_sequence().unwrap_or(&default);

        let new_seq = bl_seq.iter().chain(br_seq).map(|v| v.clone()).collect();

        let new_seq = Self::remove_fields_duplicate(new_seq);

        let new_body = YmlValue::Sequence(new_seq);
        return new_body;
    }

    fn merge_mappings(b_left: &YmlValue, b_right: &YmlValue) -> YmlValue {
        let type_left = b_left
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("string");
        let type_right = b_right
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("string");

        if type_left != type_right {
            warn!(
                "can't merge types of body: {:?} and {:?}; returning left",
                b_left, b_right
            );
            return b_left.clone();
        }

        let mut type_copy = b_left.clone();

        if type_left == "array" {
            Self::merge_mappings_type_array(b_left, b_right, &mut type_copy);
        }

        if type_left == "object" {
            Self::merge_mappings_type_object(b_left, b_right, &mut type_copy);
        }

        Self::merge_mappings_descriptions(b_left, b_right, &mut type_copy);

        Self::merge_mappings_enum(b_left, b_right, &mut type_copy);

        return type_copy;
    }

    fn merge_mappings_type_array(b_left: &YmlValue, b_right: &YmlValue, into: &mut YmlValue) {
        let l_items_opt = b_left.get("items");
        let r_items_opt = b_right.get("items");

        let merged_items_opt = l_items_opt
            .iter()
            .chain(r_items_opt.iter())
            .filter(|v| {
                if !v.is_mapping() {
                    warn!("Array items can be mapping only! Got {:?}", v);
                }
                v.is_mapping()
            }) // only mapping is allowed for items
            .map(|v| (*v).clone())
            .reduce(|a, b| Self::merge_two_types(&a, &b));

        if let Some(merged_items) = merged_items_opt {
            // if atleast one items is there
            if let Some(m) = into.as_mapping_mut() {
                // alway true
                m.insert(YmlValue::String("items".into()), merged_items);
            }
        }
    }

    fn merge_mappings_type_object(b_left: &YmlValue, b_right: &YmlValue, into: &mut YmlValue) {
        let l_fields_opt = b_left.get("fields");
        let r_fields_opt = b_right.get("fields");

        let merged_fields_opt = l_fields_opt
            .iter()
            .chain(r_fields_opt.iter())
            .filter(|v| {
                if !v.is_sequence() {
                    warn!("Object fields is not sequence! Obj: {:?}", v);
                }
                v.is_sequence()
            }) // fields sequence is only allowed fields type
            .map(|v| (*v).clone())
            .reduce(|a, b| Self::merge_two_types(&a, &b));
        if let Some(merged_fields) = merged_fields_opt {
            // if at least one fields is presented and is sequence
            if let Some(m) = into.as_mapping_mut() {
                // always true
                m.insert(YmlValue::String("fields".into()), merged_fields);
            }
        }
    }

    fn merge_mappings_descriptions(b_left: &YmlValue, b_right: &YmlValue, into: &mut YmlValue) {
        let description_l = b_left.get("description");
        let description_r = b_right.get("description");

        let description_opt = description_l
            .iter()
            .chain(description_r.iter())
            .flat_map(|v| v.as_str())
            .map(|s| s.to_string())
            .reduce(|d1, d2| {
                let mut res = String::with_capacity(d1.len() + d2.len() + "; ".len());

                res.push_str(&d1);
                res.push_str("; ");
                res.push_str(&d2);

                res
            });

        if let Some(description) = description_opt {
            if let Some(m) = into.as_mapping_mut() {
                m.insert(
                    YmlValue::String("description".to_string()),
                    YmlValue::String(description),
                );
            }
        }
    }

    fn merge_mappings_enum(b_left: &YmlValue, b_right: &YmlValue, into: &mut YmlValue) {
        let l_enum = b_left.get("enum").and_then(|es| es.as_sequence());
        let r_enum = b_right.get("enum").and_then(|es| es.as_sequence());

        let enums: Vec<_> = l_enum
            .iter()
            .chain(r_enum.iter())
            .flat_map(|v| v.iter())
            .map(|v| v.clone())
            .unique()
            .collect();

        if let Some(m) = into.as_mapping_mut() && enums.len() > 0 {
            // always true
            m.insert(YmlValue::String("enum".into()), YmlValue::Sequence(enums));
        }
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

#[cfg(test)]
mod test {
    use serde_yaml_ng::{Value as YmlValue, Mapping as YmlMapping};

    use crate::endpoints::parser::Endpoint;
    #[test]
    fn test_merge_mappings_enum() {
        let left_val: YmlValue = serde_yaml_ng::from_str(
            r#"
                enum: ["1", "2", "3"]
            "#,
        )
        .unwrap();

        let right_val: YmlValue = serde_yaml_ng::from_str(
            r#"
                enum: ["3", "4", "5", "6", "7"]
            "#
        )
        .unwrap();

        let mut into = YmlValue::Mapping(YmlMapping::new());

        Endpoint::merge_mappings_enum(&left_val, &right_val, &mut into);

        assert_eq!(into.get("enum").unwrap().as_sequence().unwrap().len(), 7);

        let mut into = YmlValue::Mapping(YmlMapping::new());
        let null = YmlValue::Null;
        
        Endpoint::merge_mappings_enum(&null, &right_val, &mut into);
        
        assert_eq!(into.get("enum").unwrap().as_sequence().unwrap().len(), 5);

        let mut into = YmlValue::Mapping(YmlMapping::new());

        Endpoint::merge_mappings_enum(&left_val, &null, &mut into);

        assert_eq!(into.get("enum").unwrap().as_sequence().unwrap().len(), 3);

        let mut into = YmlValue::Mapping(YmlMapping::new());
        
        Endpoint::merge_mappings_enum(&null, &null, &mut into);

        assert!(into.get("enum").is_none());
    }

    #[test]
    fn test_merge_mappings_descriptions() {
        let left_val: YmlValue = serde_yaml_ng::from_str(
            r#"
                description: some
            "#,
        )
        .unwrap();

        let right_val: YmlValue = serde_yaml_ng::from_str(
            r#"
                description: another
            "#
        )
        .unwrap();

        let mut into = YmlValue::Mapping(YmlMapping::new());

        Endpoint::merge_mappings_descriptions(&left_val, &right_val, &mut into);

        assert_eq!(into.get("description").unwrap(), "some; another");

        let mut into = YmlValue::Mapping(YmlMapping::new());
        let null = YmlValue::Null;
        
        Endpoint::merge_mappings_descriptions(&null, &right_val, &mut into);
        
        assert_eq!(into.get("description").unwrap(), "another");

        let mut into = YmlValue::Mapping(YmlMapping::new());

        Endpoint::merge_mappings_descriptions(&left_val, &null, &mut into);

        assert_eq!(into.get("description").unwrap(), "some");

        let mut into = YmlValue::Mapping(YmlMapping::new());
        
        Endpoint::merge_mappings_descriptions(&null, &null, &mut into);

        assert!(into.get("description").is_none());
    }
}
