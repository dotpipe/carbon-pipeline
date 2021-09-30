#[macro_use]
extern crate carbon_core;
extern crate log;
extern crate serde_json;

use carbon_core::{
    handlebars_helpers::{syntax_clear, syntax_wrapper},
    log::setup as log_setup,
    modules::{Config, Listener, Return},
};
use serde_json::Value;
use std::collections::HashMap;

fn value_to_string(item: &Value) -> Result<String, String> {
    if item.is_string() {
        Ok(format!(r#""{}""#, item.as_str().unwrap()))
    } else if item.is_f64() {
        Ok(format!("{}", item.as_f64().unwrap()))
    } else if item.is_i64() {
        Ok(format!("{}", item.as_i64().unwrap()))
    } else if item.is_u64() {
        Ok(format!("{}", item.as_u64().unwrap()))
    } else if item.is_boolean() {
        if item.as_bool().unwrap() {
            Ok("true".to_string())
        } else {
            Ok("false".to_string())
        }
    } else if item.is_null() {
        Ok("null".to_string())
    } else {
        Err("Cannot convert Array and Object to string.".to_string())
    }
}

fn switch<F: Fn(Return)>(listener: Listener, send: F, config: Config) {
    if !cfg!(test) {
        log_setup();
    }

    match config.params {
        Some(value) => match value.as_array() {
            Some(cases) => {
                register_helpers!(handlebars);

                let cases_map = {
                    let mut map = HashMap::new();
                    for case in cases {
                        if let Some(case) = case.as_object() {
                            if let Some(condition) = case.get("condition") {
                                if let Some(attach) = case.get("attach") {
                                    let template = syntax_wrapper(condition.as_str().unwrap());

                                    match handlebars.register_template_string(
                                        &template.clone(),
                                        &template.clone(),
                                    ) {
                                        Ok(_) => {}
                                        Err(err) => {
                                            panic!("{}", err);
                                        }
                                    };

                                    map.insert(
                                        template.clone(),
                                        attach.as_str().unwrap().to_string(),
                                    );
                                }
                            } else if let Some(operator) = case.get("operator") {
                                let left = match value_to_string(case.get("left").unwrap()) {
                                    Ok(value) => value,
                                    Err(err) => panic!("{}", err),
                                };
                                let right = match value_to_string(case.get("right").unwrap()) {
                                    Ok(value) => value,
                                    Err(err) => panic!("{}", err),
                                };
                                let attach =
                                    case.get("attach").unwrap().as_str().unwrap().to_string();

                                let template = syntax_wrapper(&format!(
                                    "{} {} {}",
                                    operator.as_str().unwrap(),
                                    syntax_clear(&left),
                                    syntax_clear(&right)
                                ));

                                match handlebars
                                    .register_template_string(&template.clone(), &template.clone())
                                {
                                    Ok(_) => {}
                                    Err(err) => {
                                        panic!("{}", err);
                                    }
                                };

                                map.insert(template.clone(), attach);
                            }
                        }
                    }
                    map
                };

                for request in listener {
                    let mut sent = false;
                    for (template, attach) in cases_map.clone() {
                        match handlebars.render(&template, &request.payload.clone().unwrap()) {
                            Ok(value) if value == "true" => {
                                send(Return {
                                    payload: request.payload.clone(),
                                    attach: Some(String::from(attach)),
                                    trace_id: request.trace_id,
                                });
                                sent = true;
                                break;
                            }
                            res => {
                                log::error!("Res: {:?}", res);
                            }
                        }
                    }

                    if !sent {
                        send(Return {
                            payload: request.payload.clone(),
                            attach: config.default_attach.clone(),
                            trace_id: request.trace_id,
                        });
                    }
                }
            }
            _ => {}
        },
        _ => {}
    };
}

create_module!(switch);

#[cfg(test)]
mod tests {
    use carbon_core::modules::*;
    use serde_json::json;

    #[test]
    fn condition() {
        let config = Config {
            reference: "test".parse().unwrap(),
            params: Some(json!([
                {
                    "condition": "eq num 1",
                    "attach": "foo"
                },
                {
                    "condition": "eq num 2",
                    "attach": "bar",
                }
            ])),
            producer: None,
            default_attach: None,
        };
        let payload = Ok(Some(json!({
            "num": 1
        })));
        let compare = Some("foo".to_string());
        create_module_assert_eq_attach!(crate::switch, config, payload, compare);
    }

    #[test]
    fn complex() {
        let config = Config {
            reference: "test".parse().unwrap(),
            params: Some(json!([
                {
                    "operator": "eq",
                    "left": "{{num}}",
                    "right": "1",
                    "attach": "foo"
                },
                {
                    "operator": "eq",
                    "left": "{{num}}",
                    "right": 1,
                    "attach": "bar"
                },
                {
                    "operator": "eq",
                    "left": "{{num}}",
                    "right": 2,
                    "attach": "qux"
                }
            ])),
            producer: None,
            default_attach: Some("none".to_string()),
        };

        let payload = Ok(Some(json!({
            "num": 2
        })));
        let compare = Some("qux".to_string());
        let config_copy = config.clone();
        create_module_assert_eq_attach!(crate::switch, config_copy, payload, compare);

        let payload = Ok(Some(json!({
            "num": "1"
        })));
        let compare = Some("foo".to_string());
        let config_copy = config.clone();
        create_module_assert_eq_attach!(crate::switch, config_copy, payload, compare);

        let payload = Ok(Some(json!({
            "num": 1
        })));
        let compare = Some("bar".to_string());
        create_module_assert_eq_attach!(crate::switch, config, payload, compare);
    }

    #[test]
    fn condition_types() {
        let config = Config {
            reference: "test".parse().unwrap(),
            params: Some(json!([
                {
                    "condition": "eq num (to_string 1)",
                    "attach": "foo"
                },
                {
                    "condition": "eq num (to_string 2)",
                    "attach": "bar",
                },
                {
                    "condition": "eq 3.5 (to_number num)",
                    "attach": "qux",
                }
            ])),
            producer: None,
            default_attach: None,
        };

        let payload = Ok(Some(json!({
            "num": "3.5"
        })));
        let compare = Some("qux".to_string());
        create_module_assert_eq_attach!(crate::switch, config, payload, compare);
    }
}
