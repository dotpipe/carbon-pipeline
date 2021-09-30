use crate::co2_struct::{Co2, Payload, Step};
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub fn cyml_to_yaml(content: String) -> String {
    let mut text = content.clone();
    let mut occurrences = HashMap::new();

    let re = Regex::new(r"(?P<script>(\{\{)((.|\n)*?)(}}))").unwrap();
    for caps in re.captures_iter(&content) {
        occurrences.entry(caps["script"].to_string()).or_insert(());
    }

    for (occurrence, _) in occurrences.iter() {
        let no_quotes = occurrence.replace("\"", "\\\"");
        let new_value = format!("\"{}\"", no_quotes);
        text = text.replace(&occurrence.as_str(), &new_value);
    }

    text
}

pub fn import_file(pathname: &str) -> Result<Co2, String> {
    let path = Path::new(pathname);
    let raw = fs::read_to_string(path).expect(&format!(
        "Co2: Something went wrong reading the file: {}",
        pathname
    ));

    let content = match path.extension() {
        Some(ext) if ext == "cyml" => cyml_to_yaml(raw),
        None => return Err(format!("Co2: Invalid format")),
        Some(ext) if ext == "yml" || ext == "yaml" => raw,
        Some(ext) => return Err(format!("Co2: Invalid file {:?}", ext)),
    };

    match serde_yaml::from_str(&content) {
        Ok(conf) => render(conf),
        Err(err) => Err(err.to_string()),
    }
}

pub fn render(origin: Co2) -> Result<Co2, String> {
    let mut pipeline = Vec::new();

    for step in origin.pipeline {
        if step.module.is_some() && step.clone().module.unwrap() == "payload" {
            pipeline.push(Step {
                module: step.clone().module,
                params: step.clone().params,
                payload: None,
                reference: step.clone().reference,
                producer: step.clone().producer,
                attach: step.clone().attach,
            });
            continue;
        }

        if step.module.is_none() {
            pipeline.push(Step {
                module: Some("payload".to_string()),
                params: step.clone().params,
                payload: step.clone().payload,
                reference: step.clone().reference,
                producer: step.clone().producer,
                attach: step.clone().attach,
            });
            continue;
        }

        if step.payload.is_none() {
            pipeline.push(step.clone());
            continue;
        }

        if step.clone().payload.unwrap().request.is_some() {
            pipeline.push(Step {
                module: Some("payload".to_string()),
                params: None,
                payload: Some(Payload {
                    request: step.clone().payload.unwrap().request,
                    response: None,
                }),
                reference: match step.clone().reference {
                    Some(reference) => Some(reference),
                    None => None,
                },
                producer: step.clone().producer,
                attach: None,
            });

            if step.clone().payload.unwrap().response.is_none() {
                pipeline.push(Step {
                    module: step.clone().module,
                    params: step.clone().params,
                    payload: None,
                    reference: None,
                    producer: None,
                    attach: step.clone().attach,
                });
            }
        }

        if step.clone().payload.unwrap().response.is_some() {
            pipeline.push(Step {
                module: step.clone().module,
                params: step.clone().params,
                payload: None,
                reference: None,
                producer: None,
                attach: None,
            });

            pipeline.push(Step {
                module: Some("payload".to_string()),
                params: None,
                payload: Some(Payload {
                    request: None,
                    response: step.clone().payload.unwrap().response,
                }),
                reference: None,
                producer: None,
                attach: step.clone().attach,
            });
        }
    }

    Ok(Co2 {
        version: origin.version,
        modules: origin.modules,
        pipeline,
    })
}

#[cfg(test)]
mod tests {
    extern crate assert_type_eq;

    use crate::cyml_import::*;

    #[test]
    fn import_file_yml() {
        assert!(match import_file("test/example.yml") {
            Ok(conf) if conf.version == Some(String::from("test")) => true,
            _ => false,
        })
    }

    #[test]
    fn import_file_cyml() {
        assert!(match import_file("test/example.cyml") {
            Ok(conf) if conf.version == Some(String::from("test")) => true,
            _ => false,
        })
    }

    #[test]
    fn import_file_other_format() {
        assert!(match import_file("test/example.xxx") {
            Ok(_) => false,
            Err(_) => true,
        })
    }
}
