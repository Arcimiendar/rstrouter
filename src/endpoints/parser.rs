use itertools::Itertools;
use log::warn;
use rstmytype::{ApiEndpoint, ApiEndpointMethod, ApiProject};
use serde_yaml_ng::{Mapping as YmlMapping, Value as YmlValue};
use std::fs::{read_dir, read_to_string};
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

struct SoftList<'a, T> {
    el: T,
    next: Option<&'a Self>,
}

impl<'a, T> SoftList<'a, T> {
    pub fn iter(&'a self) -> SoftListIter<'a, T> {
        SoftListIter {
            current: Some(self),
        }
    }
}

struct SoftListIter<'a, T> {
    current: Option<&'a SoftList<'a, T>>,
}

impl<'a, T> Iterator for SoftListIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.current?;
        if let Some(next) = current.next {
            self.current = Some(next);
        } else {
            self.current = None;
        }

        return Some(&current.el);
    }
}

impl EndpointsCollection {
    pub fn parse_from_dir(dsl_dir: &str) -> Self {
        let current_dir = PathBuf::from(dsl_dir);
        let current_url = "";
        let mut endpoints_acc = Vec::new();
        Self::parse_from_dir_rec(current_url, &current_dir, None, &mut endpoints_acc);
        Self {
            endpoints: endpoints_acc,
        }
    }

    fn parse_from_dir_rec(
        current_url: &str,
        current_dir: &PathBuf,
        in_guard_list: Option<&SoftList<Guard>>,
        endpoints_acc: &mut Vec<Endpoint>,
    ) {
        let guard_list: Option<&SoftList<Guard>>;
        let local_soft_list: SoftList<Guard>;

        if let Some(guard) = Guard::parse_guard_from_dir(current_dir) {
            local_soft_list = SoftList {
                el: guard,
                next: in_guard_list,
            };
            guard_list = Some(&local_soft_list);
        } else {
            guard_list = in_guard_list;
        }

        read_dir(current_dir)
            .ok()
            .iter_mut()
            .flat_map(|r| r.into_iter())
            .flat_map(|e| e.ok())
            .for_each(|f| {
                let f_path = f.path();
                if !f_path.is_dir() {
                    return;
                }

                if f.file_name() == "POST" {
                    Endpoint::parse_from_dir_rec(
                        &ApiEndpointMethod::Post,
                        current_url,
                        current_url,
                        &f_path,
                        guard_list,
                        endpoints_acc,
                    );
                    return;
                }

                if f.file_name() == "GET" {
                    Endpoint::parse_from_dir_rec(
                        &ApiEndpointMethod::Get,
                        current_url,
                        current_url,
                        &f_path,
                        guard_list,
                        endpoints_acc,
                    );
                    return;
                }

                let Some(new_current_url) = f
                    .file_name()
                    .to_str()
                    .map(|fname| format!("{}/{}", current_url, fname))
                else {
                    return;
                };

                Self::parse_from_dir_rec(&new_current_url, &f_path, guard_list, endpoints_acc);
            });
    }
}

impl Endpoint {
    fn parse_from_dir_rec(
        method: &ApiEndpointMethod,
        current_url: &str,
        tag: &str,
        current_dir: &PathBuf,
        in_guard_list: Option<&SoftList<Guard>>,
        endpoints_acc: &mut Vec<Endpoint>,
    ) {
        let guard_list: Option<&SoftList<Guard>>;
        let local_soft_list: SoftList<Guard>;

        if let Some(guard) = Guard::parse_guard_from_dir(current_dir) {
            local_soft_list = SoftList {
                el: guard,
                next: in_guard_list,
            };
            guard_list = Some(&local_soft_list);
        } else {
            guard_list = in_guard_list;
        }

        read_dir(current_dir)
            .ok()
            .iter_mut()
            .flat_map(|r| r.into_iter())
            .flat_map(|e| e.ok())
            .for_each(|f| {
                let f_path = f.path();

                if !f_path.is_dir() {
                    if !f_path.is_file() {
                        warn!(
                            "{} in {} is not a file nor dir",
                            f_path.display(),
                            current_dir.display()
                        );
                        return;
                    }

                    if let Some(f_name) = f_path.file_name().and_then(|f| f.to_str()) {
                        if f_name.starts_with(".guard") {
                            return;
                        }
                    }

                    if let Some(endpoint) =
                        Self::parse_from_file(&f_path, tag, guard_list, method, current_url)
                    {
                        endpoints_acc.push(endpoint);
                    }

                    return;
                }

                let Some(new_current_url) = f
                    .file_name()
                    .to_str()
                    .map(|fname| format!("{}/{}", current_url, fname))
                else {
                    return;
                };

                Self::parse_from_dir_rec(
                    method,
                    &new_current_url,
                    tag,
                    &f_path,
                    guard_list,
                    endpoints_acc,
                );
            });
    }

    fn parse_from_file(
        file: &PathBuf,
        tag: &str,
        guard_list: Option<&SoftList<Guard>>,
        method: &ApiEndpointMethod,
        url_path: &str,
    ) -> Option<Self> {
        let content = read_to_string(file).ok()?;
        let Some(yml_content) = serde_yaml_ng::from_str(&content).ok() else {
            warn!("Endpoint {} has bad yml content", file.display());
            return None;
        };

        let f_name = file.file_stem()?.to_str()?;

        let mut obj = Self {
            guards: guard_list
                .iter()
                .flat_map(|l| l.iter())
                .map(|g| g.clone())
                .collect(),
            tag: tag.to_string(),
            method: method.clone(),
            yml_content: yml_content,
            url_path: format!("{}/{}", url_path, f_name),
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
        // todo: unittest it
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
            return b_right.clone();
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

        if let Some(m) = into.as_mapping_mut()
            && enums.len() > 0
        {
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
    use rstmytype::ApiEndpointMethod;
    use serde_yaml_ng::{Mapping as YmlMapping, Value as YmlValue};

    use crate::endpoints::parser::{Endpoint, EndpointsCollection};
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
            "#,
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
            "#,
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

    #[test]
    fn test_merge_mappings_type_object() {
        let mut into = YmlValue::Mapping(serde_yaml_ng::Mapping::new());
        let object_left: YmlValue = serde_yaml_ng::from_str(
            r#"
                fields: 
                  - field: test
                    type: string
                  - field: test1
                    type: string
            "#,
        )
        .unwrap();
        let object_right: YmlValue = serde_yaml_ng::from_str(
            r#"
                fields:
                  - field: test
                    type: string
                  - field: test2
                    type: string
            "#,
        )
        .unwrap();

        Endpoint::merge_mappings_type_object(&object_left, &object_right, &mut into);
        assert_eq!(
            into.as_mapping()
                .unwrap()
                .get("fields")
                .unwrap()
                .as_sequence()
                .unwrap()
                .len(),
            3
        );
    }

    #[test]
    fn test_merge_mappings_type_array() {
        let mut into = YmlValue::Mapping(serde_yaml_ng::Mapping::new());
        let object_left: YmlValue = serde_yaml_ng::from_str(
            r#"
                items:
                  type: object
                  fields:
                    - field: hello
                      type: string
                    - field: world
                      type: string
                      description: hello
                  description: hello
            "#,
        )
        .unwrap();
        let object_right: YmlValue = serde_yaml_ng::from_str(
            r#"
                items:
                  type: object
                  description: world
                  fields:
                    - field: hello
                      type: string
                    - field: world
                      type: string
                      description: world
            "#,
        )
        .unwrap();
        Endpoint::merge_mappings_type_array(&object_left, &object_right, &mut into);
        assert_eq!(
            into.get("items").unwrap().get("description").unwrap(),
            "hello; world"
        );
    }

    #[test]
    fn test_merge_mappings() {
        let object_left: YmlValue = serde_yaml_ng::from_str(
            r#"
                type: string
                description: hello!
                enum: ['1', '2']
            "#,
        )
        .unwrap();
        let object_right: YmlValue = serde_yaml_ng::from_str(
            r#"
                type: string
                description: world!
                enum: ['2', '3']
            "#,
        )
        .unwrap();
        let res = Endpoint::merge_mappings(&object_left, &object_right);
        assert_eq!(res.get("description").unwrap(), "hello!; world!");
        assert_eq!(res.get("type").unwrap(), "string");
        assert_eq!(res.get("enum").unwrap().as_sequence().unwrap().len(), 3);

        let object_right: YmlValue = serde_yaml_ng::from_str(
            r#"
                type: number
                description: world!
            "#,
        )
        .unwrap();
        let res = Endpoint::merge_mappings(&object_left, &object_right);
        assert_eq!(res.get("type").unwrap(), "string");
        assert_eq!(res.get("description").unwrap(), "hello!");

        let object_left: YmlValue = serde_yaml_ng::from_str(
            r#"
                type: object
                description: hello!
            "#,
        )
        .unwrap();
        let object_right: YmlValue = serde_yaml_ng::from_str(
            r#"
                type: object
                description: world!
            "#,
        )
        .unwrap();
        let res = Endpoint::merge_mappings(&object_left, &object_right);
        assert_eq!(res.get("description").unwrap(), "hello!; world!");

        let object_left: YmlValue = serde_yaml_ng::from_str(
            r#"
                type: array
                description: hello!
            "#,
        )
        .unwrap();
        let object_right: YmlValue = serde_yaml_ng::from_str(
            r#"
                type: array
                description: world!
            "#,
        )
        .unwrap();
        let res = Endpoint::merge_mappings(&object_left, &object_right);
        assert_eq!(res.get("description").unwrap(), "hello!; world!");
    }

    #[test]
    fn test_merge_sequence() {
        let b_left = serde_yaml_ng::from_str(
            r#"
                - field: test1
                  type: number
                - field: test2
                  type: string
            "#,
        )
        .unwrap();
        let b_right = serde_yaml_ng::from_str(
            r#"
                - field: test2
                  type: string
                - field: test3
                  type: object
            "#,
        )
        .unwrap();
        let res = Endpoint::merge_sequences(&b_left, &b_right);
        assert_eq!(res.as_sequence().unwrap().len(), 3);
    }

    #[test]
    fn test_merge_seq_with_map() {
        let b_left = serde_yaml_ng::from_str(
            r#"
                - field: test2
                  type: string
                - field: test3
                  type: object
            "#,
        )
        .unwrap();
        let b_right = serde_yaml_ng::from_str(
            r#"
                type: object
                fields:
                  - field: test1
                    type: number
                  - field: test2
                    type: string
            "#,
        )
        .unwrap();
        let res = Endpoint::merge_seq_with_map(&b_left, &b_right);
        assert_eq!(res.get("fields").unwrap().as_sequence().unwrap().len(), 3);
    }

    #[test]
    fn test_merge_two_types() {
        let null = YmlValue::Null;
        let seq = serde_yaml_ng::from_str(
            r#"
                - field: test2
                  type: string
                - field: test3
                  type: object
            "#,
        )
        .unwrap();
        let obj = serde_yaml_ng::from_str(
            r#"
                type: object
                fields:
                  - field: test1
                    type: number
                  - field: test2
                    type: string
            "#,
        )
        .unwrap();
        let number = serde_yaml_ng::from_str("5").unwrap();

        let res = Endpoint::merge_two_types(&null, &obj);
        assert_eq!(res, obj);

        let res = Endpoint::merge_two_types(&seq, &null);
        assert_eq!(res, seq);

        let res = Endpoint::merge_two_types(&seq, &number);
        assert_eq!(res, seq);

        let res = Endpoint::merge_two_types(&number, &seq);
        assert_eq!(res, seq);

        let res = Endpoint::merge_two_types(&number, &null);
        assert_eq!(res, number);

        let res = Endpoint::merge_two_types(&seq, &obj);
        assert_eq!(res.get("fields").unwrap().as_sequence().unwrap().len(), 3);

        let res = Endpoint::merge_two_types(&obj, &seq);
        assert_eq!(res.get("fields").unwrap().as_sequence().unwrap().len(), 3);
    }

    #[test]
    fn smoke_test_fold_is_parsing() {
        let endp = EndpointsCollection::parse_from_dir("./unittest_dsl");
        assert_eq!(endp.endpoints.len(), 2);

        let first_endp = &endp.endpoints[0];
        assert_eq!(first_endp.method, ApiEndpointMethod::Post);
        assert_eq!(first_endp.guards.len(), 1);

        let second_endp = &endp.endpoints[1];
        assert_eq!(second_endp.method, ApiEndpointMethod::Get);
        assert_eq!(second_endp.guards.len(), 2);
    }
}
