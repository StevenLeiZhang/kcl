//! Copyright The KCL Authors. All rights reserved.

use crate::*;
use std::boxed::Box;
use std::cell::RefCell;
use std::rc::Rc;

pub const KCL_PRIVATE_VAR_PREFIX: &str = "_";
const LIST_DICT_TEMP_KEY: &str = "$";
const SCHEMA_TYPE_META_ATTR: &str = "_type";

/// PlanOptions denotes the configuration required to execute the KCL
/// program and the JSON/YAML planning.
#[derive(Debug, Default, Clone)]
pub struct PlanOptions {
    pub sort_keys: bool,
    pub include_schema_type_path: bool,
}

fn filter_results(ctx: &Context, key_values: &ValueRef, opts: &PlanOptions) -> Vec<ValueRef> {
    let mut results: Vec<ValueRef> = vec![];
    // Plan list value with the yaml stream format.
    if key_values.is_list() {
        let key_values_list = &key_values.as_list_ref().values;
        for key_values in key_values_list {
            results.append(&mut filter_results(ctx, key_values, opts));
        }
        results
    }
    // Plan dict value
    else if key_values.is_config() {
        // index 0 for in-line keyvalues output, index 1: for standalone keyvalues outputs
        let result = ValueRef::dict(None);
        results.push(result);
        let key_values = key_values.as_dict_ref();
        for (key, value) in &key_values.values {
            if value.is_none() && ctx.cfg.disable_none {
                continue;
            }
            if key.starts_with(KCL_PRIVATE_VAR_PREFIX) || value.is_undefined() || value.is_func() {
                continue;
            } else if value.is_schema() || value.has_key(SCHEMA_SETTINGS_ATTR_NAME) {
                let (filtered, standalone) = handle_schema(ctx, value, opts);
                if !filtered.is_empty() {
                    if standalone {
                        // if the instance is marked as 'STANDALONE', treat it as a separate one and
                        // extend it and derived STANDALONE instances to results.
                        for v in filtered {
                            results.push(v);
                        }
                    } else {
                        // else put it as the value of the key of results
                        let result = results.get_mut(0).unwrap();
                        result.dict_update_key_value(key.as_str(), filtered[0].clone());
                        // if the value has derived 'STANDALONE' instances, extend them
                        if filtered.len() > 1 {
                            for v in &filtered[1..] {
                                results.push(v.clone());
                            }
                        }
                    }
                }
            } else if value.is_dict() {
                let filtered = filter_results(ctx, value, opts);
                if !results.is_empty() {
                    let result = results.get_mut(0).unwrap();
                    if !filtered.is_empty() {
                        result.dict_update_key_value(key.as_str(), filtered[0].clone());
                    }
                    // if the value has derived 'STANDALONE' instances, extend them
                    if filtered.len() > 1 {
                        for v in &filtered[1..] {
                            results.push(v.clone());
                        }
                    }
                }
            } else if value.is_list() {
                let mut filtered_list: Vec<ValueRef> = vec![];
                let mut standalone_list: Vec<ValueRef> = vec![];
                let mut ignore_schema_count = 0;
                let list_value = value.as_list_ref();
                for v in &list_value.values {
                    if v.is_schema() || v.has_key(SCHEMA_SETTINGS_ATTR_NAME) {
                        let (filtered, standalone) = handle_schema(ctx, v, opts);
                        if filtered.is_empty() {
                            ignore_schema_count += 1;
                            continue;
                        } else if standalone {
                            for v in filtered {
                                standalone_list.push(v);
                            }
                        } else {
                            for v in filtered {
                                filtered_list.push(v);
                            }
                        }
                    } else if v.is_dict() {
                        let filtered = filter_results(ctx, v, opts);
                        for v in filtered {
                            filtered_list.push(v);
                        }
                    } else if v.is_none() && ctx.cfg.disable_none {
                        continue;
                    } else if !v.is_undefined() {
                        let list_dict = ValueRef::dict(Some(&[(LIST_DICT_TEMP_KEY, v)]));
                        let filtered = filter_results(ctx, &list_dict, opts);
                        if !filtered.is_empty() {
                            if let Some(v) = filtered[0].get_by_key(LIST_DICT_TEMP_KEY) {
                                filtered_list.push(v.clone());
                            }
                        }
                        if filtered.len() > 1 {
                            for v in &filtered[1..] {
                                results.push(v.clone());
                            }
                        }
                    }
                }
                let schema_in_list_count = ignore_schema_count + standalone_list.len();
                let value = &value.as_list_ref().values;
                // Plan empty list to values.
                if value.is_empty() && !ctx.cfg.disable_empty_list {
                    let result = results.get_mut(0).unwrap();
                    result.dict_update_key_value(key.as_str(), ValueRef::list(None));
                }
                if schema_in_list_count < value.len() {
                    let result = results.get_mut(0).unwrap();
                    let filtered_list: Vec<&ValueRef> = filtered_list.iter().collect();
                    let filtered_list = filtered_list.as_slice();
                    let filtered_list = ValueRef::list(Some(filtered_list));
                    result.dict_update_key_value(key.as_str(), filtered_list);
                }
                for v in standalone_list {
                    results.push(v);
                }
            } else {
                let result = results.get_mut(0).unwrap();
                result.dict_update_key_value(key.as_str(), value.clone());
            }
        }
        results
            .iter()
            .enumerate()
            .filter(|(index, r)| *index == 0 || !r.is_planned_empty())
            .map(|v| v.1)
            .cloned()
            .collect()
    } else {
        results
    }
}

fn handle_schema(ctx: &Context, value: &ValueRef, opts: &PlanOptions) -> (Vec<ValueRef>, bool) {
    let mut filtered = filter_results(ctx, value, opts);
    if filtered.is_empty() {
        return (filtered, false);
    }
    // Deal schema type meta attribute and add the attribute with the type string value
    // into the planned object.
    if opts.include_schema_type_path {
        if let Some(v) = filtered.get_mut(0) {
            if v.is_config() {
                v.dict_update_key_value(
                    SCHEMA_TYPE_META_ATTR,
                    ValueRef::str(&value_type_path(value)),
                );
            }
        }
    }
    let settings = SCHEMA_SETTINGS_ATTR_NAME;
    let output_type = SETTINGS_OUTPUT_KEY;
    let path = format!("{settings}.{output_type}");
    let output_type_option = value.get_by_path(&path);
    if let Some(ref output_type) = output_type_option {
        if output_type.str_equal(SETTINGS_OUTPUT_IGNORE) {
            if filtered.is_empty() {
                return (filtered, false);
            } else {
                return (filtered[1..].to_vec(), true);
            }
        }
    }
    let mut standalone = false;
    if let Some(ref output_type) = output_type_option {
        if output_type.str_equal(SETTINGS_OUTPUT_STANDALONE) {
            standalone = true;
        }
    }
    (filtered, standalone)
}

/// Returns the type path of the runtime value `v`.
fn value_type_path(v: &ValueRef) -> String {
    let path = format!("{SCHEMA_SETTINGS_ATTR_NAME}.{SETTINGS_SCHEMA_TYPE_KEY}");
    match v.get_by_path(&path) {
        Some(type_path) => match &*type_path.rc.borrow() {
            Value::str_value(ty_str) => {
                let parts: Vec<&str> = ty_str.rsplit('.').collect();
                match parts.first() {
                    Some(v) => v.to_string(),
                    None => v.type_str(),
                }
            }
            _ => v.type_str(),
        },
        None => v.type_str(),
    }
}

impl ValueRef {
    fn is_planned_empty(&self) -> bool {
        (self.is_dict() && !self.is_truthy()) || self.is_undefined()
    }

    pub fn plan_to_json_string(&self, ctx: &mut Context) -> String {
        let result = self.filter_results(ctx);
        if result.is_planned_empty() {
            return "".to_string();
        }
        result.to_json_string()
    }

    pub fn plan_to_yaml_string(&self, ctx: &mut Context) -> String {
        let result = self.filter_results(ctx);
        result.to_yaml_string()
    }

    /// Plan the value to the YAML string with delimiter `---`.
    pub fn plan_to_yaml_string_with_delimiter(&self, ctx: &Context) -> String {
        let results = filter_results(ctx, self, &PlanOptions::default());
        let results = results
            .iter()
            .map(|r| r.to_yaml_string())
            .collect::<Vec<String>>();
        results.join(YAML_STREAM_SEP)
    }

    /// Plan the value to JSON and YAML strings.
    pub fn plan(&self, ctx: &Context, opts: &PlanOptions) -> (String, String) {
        let json_opt = JsonEncodeOptions {
            sort_keys: opts.sort_keys,
            ..Default::default()
        };
        let yaml_opt = YamlEncodeOptions {
            sort_keys: opts.sort_keys,
            ..Default::default()
        };
        if self.is_list_or_config() {
            let results = filter_results(ctx, self, opts);
            let yaml_result = results
                .iter()
                .map(|r| {
                    r.to_yaml_string_with_options(&yaml_opt)
                        .strip_suffix('\n')
                        .unwrap()
                        .to_string()
                })
                .collect::<Vec<String>>()
                .join(YAML_STREAM_SEP);
            let mut list_result = ValueRef::list(None);
            for r in results {
                list_result.list_append(&r);
            }
            let json_result = list_result.to_json_string_with_option(&json_opt);
            (json_result, yaml_result)
        } else {
            (
                self.to_json_string_with_option(&json_opt),
                self.to_yaml_string_with_options(&yaml_opt),
            )
        }
    }

    /// Filter values using path selectors.
    pub fn filter_by_path(&self, path_selector: &[String]) -> Result<ValueRef, String> {
        if self.is_config() && !path_selector.is_empty() {
            if path_selector.len() == 1 {
                let path = &path_selector[0];
                match self.get_by_path(path) {
                    Some(value) => Ok(value),
                    None => Err(format!(
                        "invalid path select operand {path}, value not found"
                    )),
                }
            } else {
                let mut values = ValueRef::list(None);
                for path in path_selector {
                    let value = match self.get_by_path(path) {
                        Some(value) => value,
                        None => {
                            return Err(format!(
                                "invalid path select operand {path}, value not found"
                            ))
                        }
                    };
                    values.list_append(&value);
                }
                Ok(values)
            }
        } else {
            Ok(self.clone())
        }
    }

    fn filter_results(&self, ctx: &mut Context) -> ValueRef {
        match &*self.rc.borrow() {
            Value::undefined => ValueRef {
                rc: Rc::new(RefCell::new(Value::undefined)),
            },
            Value::none => ValueRef {
                rc: Rc::new(RefCell::new(Value::none)),
            },
            Value::func_value(ref v) => ValueRef {
                rc: Rc::new(RefCell::new(Value::func_value(v.clone()))),
            },
            Value::bool_value(ref v) => ValueRef {
                rc: Rc::new(RefCell::new(Value::bool_value(*v))),
            },
            Value::int_value(ref v) => ValueRef {
                rc: Rc::new(RefCell::new(Value::int_value(*v))),
            },
            Value::float_value(ref v) => ValueRef {
                rc: Rc::new(RefCell::new(Value::float_value(*v))),
            },
            Value::unit_value(ref v, _, _) => ValueRef {
                rc: Rc::new(RefCell::new(Value::float_value(*v))),
            },
            Value::str_value(ref v) => ValueRef {
                rc: Rc::new(RefCell::new(Value::str_value(v.to_string()))),
            },
            Value::list_value(ref v) => {
                let mut list = ValueRef {
                    rc: Rc::new(RefCell::new(Value::list_value(Box::new(ListValue {
                        values: vec![],
                    })))),
                };
                for x in v.values.iter() {
                    if !(x.is_undefined() || x.is_func() || ctx.cfg.disable_none && x.is_none()) {
                        list.list_append(&x.filter_results(ctx));
                    }
                }
                list
            }
            Value::dict_value(ref v) => {
                let mut dict = ValueRef {
                    rc: Rc::new(RefCell::new(Value::dict_value(Box::new(DictValue {
                        values: IndexMap::default(),
                        ops: IndexMap::default(),
                        insert_indexs: IndexMap::default(),
                        attr_map: IndexMap::default(),
                    })))),
                };
                for (key, val) in v.values.iter() {
                    if !(val.is_undefined()
                        || val.is_func()
                        || ctx.cfg.disable_none && val.is_none())
                    {
                        let v = val.filter_results(ctx);
                        dict.dict_insert(ctx, key, &v, ConfigEntryOperationKind::Override, -1);
                    }
                }
                dict
            }
            Value::schema_value(ref v) => {
                let mut schema = ValueRef {
                    rc: Rc::new(RefCell::new(Value::schema_value(Box::new(SchemaValue {
                        name: v.name.clone(),
                        pkgpath: v.pkgpath.clone(),
                        config: Box::new(DictValue {
                            values: IndexMap::default(),
                            ops: IndexMap::default(),
                            insert_indexs: IndexMap::default(),
                            attr_map: IndexMap::default(),
                        }),
                        config_keys: vec![],
                        config_meta: v.config_meta.clone(),
                        optional_mapping: v.optional_mapping.clone(),
                    })))),
                };
                for (key, val) in v.config.values.iter() {
                    if !val.is_undefined() && !val.is_func() {
                        let v = val.filter_results(ctx);
                        schema.dict_insert(ctx, key, &v, ConfigEntryOperationKind::Union, -1);
                    }
                }
                schema
            }
        }
    }
}

#[cfg(test)]
mod test_value_plan {
    use crate::{Context, ValueRef};

    use super::filter_results;

    #[test]
    fn test_filter_results() {
        let ctx = Context::new();
        let dict1 = ValueRef::dict_int(&[("k1", 1)]);
        let dict2 = ValueRef::dict_int(&[("k2", 2)]);
        let dict3 = ValueRef::dict_int(&[("k3", 3)]);
        let dict_list = vec![&dict1, &dict2, &dict3];
        let list_data = ValueRef::list(Some(&dict_list));
        assert_eq!(
            filter_results(&ctx, &list_data, &Default::default()),
            dict_list
                .iter()
                .map(|v| v.deep_copy())
                .collect::<Vec<ValueRef>>()
        );
        for dict in dict_list {
            assert_eq!(
                filter_results(&ctx, dict, &Default::default()),
                vec![dict.deep_copy()]
            );
        }
    }

    #[test]
    fn test_filter_by_path() {
        let dict = ValueRef::dict_int(&[("k1", 1)]);
        assert_eq!(
            dict.filter_by_path(&[]).unwrap(),
            ValueRef::dict_int(&[("k1", 1)]),
        );
        assert_eq!(
            dict.filter_by_path(&["k1".to_string()]).unwrap(),
            ValueRef::int(1)
        );
        assert_eq!(
            dict.filter_by_path(&["k1".to_string(), "k1".to_string()])
                .unwrap(),
            ValueRef::list_int(&[1, 1])
        );
        assert_eq!(
            dict.filter_by_path(&["err_path".to_string()])
                .err()
                .unwrap(),
            "invalid path select operand err_path, value not found"
        );
        assert_eq!(
            dict.filter_by_path(&["err_path.to".to_string()])
                .err()
                .unwrap(),
            "invalid path select operand err_path.to, value not found"
        );
    }
}
