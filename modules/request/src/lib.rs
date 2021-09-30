#[macro_use]
extern crate carbon_core;

use std::collections::HashMap;
use std::convert::TryInto;

use carbon_core::log::setup as log_setup;
use carbon_core::modules::{Config, Listener, Response as CoreResponse, Speaker, TraceId, ID};
use reqwest::{header::HeaderMap, Client, Method};
use serde_json::{json, Map, Value};
use tokio::runtime::Runtime;

pub struct ContentRequest {
    pub headers: Option<Map<String, Value>>,
    pub body: Option<String>,
    pub status_code: Option<u16>,
}

impl ContentRequest {
    pub fn from_value(value: Value) -> Self {
        let headers = if let Some(headers) = value["headers"].as_object() {
            Some(headers.clone())
        } else {
            None
        };

        let status_code = if let Some(status_code) = value["status_code"].as_u64() {
            Some(status_code as u16)
        } else {
            None
        };

        let body = Some(value["body"].to_string());

        Self {
            headers,
            body,
            status_code,
        }
    }
}

fn header_map_to_hash_map(header_map: HeaderMap) -> HashMap<String, String> {
    let mut result = HashMap::new();

    for (key, value) in header_map {
        if let Some(key) = key {
            let key = key.as_str().to_string();
            result.insert(
                key.as_str().to_string(),
                match value.to_str() {
                    Ok(value) => value.to_string(),
                    Err(_) => "".to_string(),
                },
            );
        }
    }

    result
}

fn map_to_header_map(map: &Map<String, Value>) -> HeaderMap {
    let mut hash_map = HashMap::new();
    for (key, value) in map {
        hash_map.insert(
            key.clone(),
            match value.as_str() {
                Some(value) => value.to_string(),
                None => "".to_string(),
            },
        );
    }
    let headers: HeaderMap = (&hash_map).try_into().expect("valid headers");
    headers
}

async fn generic_request(method: String, url: String, header: HeaderMap, body: String) -> Value {
    let method = Method::from_bytes(method.as_bytes()).unwrap();

    match Client::new()
        .request(method, url)
        .body(body)
        .headers(header)
        .send()
        .await
    {
        Ok(response) => {
            let header_map = response.headers().clone();
            let status_code = response.status().as_u16();
            let body = match response.text().await {
                Ok(body) => Some(body),
                Err(_) => None,
            };

            json!({
                "headers": header_map_to_hash_map(header_map),
                "body": body,
                "status_code": Some(status_code),
            })
        }
        Err(err) => {
            json!({
                "error": err.to_string()
            })
        }
    }
}

fn request(id: ID, listener: Listener, speaker: Speaker, config: Config) {
    log_setup();

    let params = config.clone().params.unwrap();
    let method = params["method"]
        .as_str()
        .unwrap_or("GET")
        .to_string()
        .to_uppercase();
    let url = params["url"].as_str().unwrap_or("").to_string();
    let header = match params["header"].as_object() {
        Some(header) => map_to_header_map(header),
        None => HeaderMap::new(),
    };
    let body = match params["body"].as_str() {
        Some(body) => body.to_string(),
        None => "".to_string(),
    };
    let attach = match params["attach"].as_str() {
        Some(attach) => Some(attach.to_string()),
        None => None,
    };
    let trace = TraceId::global();
    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        for request_step in listener {
            if let Ok(payload) = request_step.payload {
                if let Some(payload) = payload {
                    let result = if let Some(payload) = payload.as_object() {
                        let method = match payload.get("method") {
                            Some(value) => match value.as_str() {
                                Some(value) => value.to_string().to_uppercase(),
                                None => method.clone(),
                            },
                            None => method.clone(),
                        };
                        let url = match payload.get("url") {
                            Some(value) => match value.as_str() {
                                Some(value) => value.to_string(),
                                None => url.clone(),
                            },
                            None => url.clone(),
                        };
                        let header = match payload.get("header") {
                            Some(value) => match value.as_object() {
                                Some(value) => map_to_header_map(value),
                                None => header.clone(),
                            },
                            None => header.clone(),
                        };
                        let body = match payload.get("body") {
                            Some(value) => match value.as_str() {
                                Some(value) => value.to_string(),
                                None => body.clone(),
                            },
                            None => body.clone(),
                        };

                        generic_request(method, url, header, body).await
                    } else {
                        generic_request(method.clone(), url.clone(), header.clone(), body.clone())
                            .await
                    };

                    speaker
                        .send(CoreResponse {
                            payload: Ok(Some(result)),
                            attach: attach.clone(),
                            origin: id,
                            trace_id: trace.lock().unwrap().get_trace(),
                        })
                        .unwrap();
                }
            }
        }
    });
}

create_module_raw!(request);

#[cfg(test)]
mod tests {
    use actix_web::{rt::System, web, App, HttpRequest, HttpResponse, HttpServer};
    use carbon_core::modules::*;
    use reqwest::StatusCode;
    use std::thread;
    use tokio::runtime::Runtime;

    macro_rules! create_server {
        ($port:expr, $path:expr) => {
            thread::spawn(|| {
                let rt = Runtime::new().unwrap();

                rt.block_on(async {
                    let sys = System::new("http-server-test");

                    HttpServer::new(|| {
                        App::new().service(web::resource($path).route(web::to(
                            move |_: web::Bytes, _: HttpRequest| {
                                HttpResponse::build(StatusCode::CREATED)
                                    .header("x-test", "success")
                                    .body("OK!")
                            },
                        )))
                    })
                    .bind(("127.0.0.1", $port))
                    .unwrap()
                    .run()
                    .await
                    .unwrap();

                    let _ = sys.run();
                });
            });
        };
    }

    #[tokio::test]
    async fn test_not_producer() {
        let config = Config {
            reference: "test".to_string(),
            params: Some(json!({
                "url": "http://127.0.0.1:10011/test",
                "method": "GET"
            })),
            producer: Some(false),
            default_attach: None,
        };

        create_server!(10011u16, "other");

        run_module_raw!(crate::request, config, tx, rx);

        tx.send(Request {
            origin: 1,
            trace_id: 1,
            payload: Ok(Some(json!({
                "url": "http://127.0.0.1:10011/other",
            }))),
        })
        .unwrap();

        let res = rx.recv().unwrap();
        let payload = res.payload.unwrap().unwrap();
        let headers = payload["headers"].as_object().unwrap();
        let status_code = payload["status_code"].as_i64().clone();
        let body = payload["body"].as_str().clone();
        let x_test = headers.get("x-test").unwrap().as_str();

        assert_eq!(body.unwrap(), "OK!");
        assert_eq!(status_code.unwrap(), 201);
        assert_eq!(x_test.unwrap(), "success");
    }
}
