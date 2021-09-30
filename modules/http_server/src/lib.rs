#[macro_use]
extern crate carbon_core;
extern crate serde_json;
#[macro_use]
extern crate log;

use actix_web::http::StatusCode;
use actix_web::rt::System;
use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer};
use carbon_core::{
    log::setup as log_setup,
    modules::{Config, Listener, Response, Speaker, TraceId, ID},
};
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};

#[derive(Debug)]
#[allow(dead_code)]
struct ContentRequest {
    pub body: Option<String>,
    pub headers: Option<Value>,
    pub query_string: Option<Value>,
    pub params: Option<Value>,
    pub path: Option<String>,
    pub method: Option<String>,
}

#[derive(Debug, Default)]
#[allow(dead_code)]
struct ContentResponse {
    pub headers: Option<Map<String, Value>>,
    pub body: Option<String>,
    pub status_code: Option<u16>,
}

impl ContentResponse {
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

#[derive(Debug)]
#[allow(dead_code)]
struct HttpRequestInner {
    pub trace_id: String,
    pub tx: Sender<ContentResponse>,
}

fn http_server(id: ID, listener: Listener, speaker: Speaker, config: Config) {
    if !cfg!(test) {
        log_setup();
    }

    let sys = System::new("http-server");
    let params = config.clone().params.unwrap();
    let params_clone = params.clone();
    let address = {
        if params["address"].is_string() {
            params["address"].as_str().unwrap().to_owned()
        } else if params["port"].is_number() {
            format!("127.0.0.1:{}", params["port"].as_i64().unwrap())
        } else if params["port"].is_string() {
            format!("127.0.0.1:{}", params["port"].as_str().unwrap())
        } else {
            "127.0.0.1:8080".to_owned()
        }
    };
    let requests_map = Arc::new(Mutex::new(HashMap::new()));
    let requests_map_clone = requests_map.clone();
    let trace = TraceId::global();

    HttpServer::new(move || {
        trace!("New http worker created.");
        let requests_map_inner_clone = requests_map_clone.clone();
        let speaker_clone = speaker.clone();
        let trace_clone = trace.clone();

        let mut services = Vec::new();

        let routes = if params_clone["routes"].is_array() {
            params_clone["routes"].as_array().unwrap().clone()
        } else {
            vec![params_clone.clone()]
        };

        for route in routes {
            let requests_map_inner_clone = requests_map_inner_clone.clone();
            let speaker_clone = speaker_clone.clone();
            let trace_clone = trace_clone.clone();
            let method = {
                if route["method"].is_string() {
                    route["method"].as_str().unwrap().to_owned()
                } else {
                    "ANY".to_owned()
                }
            };
            let method_any = if method == "ANY" { true } else { false };
            let default_status_code =
                if let Some(default_status_code) = route["default_status_code"].as_u64() {
                    default_status_code as u16
                } else {
                    200u16
                };
            let attach = match route["attach"].as_str() {
                Some(attach) => Some(attach.to_string()),
                None => None,
            };

            services.push(
                web::resource(route["path"].as_str().unwrap()).route(web::to(
                    move |body: web::Bytes, req: HttpRequest| {
                        if !method_any && method != req.method().as_str().to_owned() {
                            return HttpResponse::NotFound().body("");
                        }

                        let trace_id = trace_clone.lock().unwrap().get_trace();
                        trace!("Run trace: {}", &trace_id);

                        let body = {
                            let body = std::str::from_utf8(&body).unwrap();
                            body.to_string()
                        };
                        let headers = {
                            let mut headers = serde_json::Map::new();
                            for (key, value) in req.headers().iter() {
                                headers
                                    .insert(key.to_string(), Value::from(value.to_str().unwrap()));
                            }
                            Value::from(headers)
                        };
                        let query_string = {
                            let mut query_string = serde_json::Map::new();
                            let query_clean = req.query_string().to_string();
                            let query: Vec<&str> = query_clean.split("&").collect();

                            for item in query {
                                let obj: Vec<&str> = item.split("=").collect();
                                match obj.get(1) {
                                    Some(value) => query_string
                                        .insert(obj[0].to_string(), Value::from(value.to_string())),
                                    None => query_string.insert(obj[0].to_string(), Value::Null),
                                };
                            }

                            Some(Value::from(query_string))
                        };
                        let params = {
                            let mut params = serde_json::Map::new();
                            for (key, value) in req.match_info().iter() {
                                params.insert(key.to_string(), Value::from(value.to_string()));
                            }
                            Some(Value::from(params))
                        };
                        let addr = req.peer_addr().unwrap();

                        let payload = json!({
                            "headers": headers,
                            "body": body,
                            "query_string": query_string,
                            "params": params,
                            "method": req.method().to_string(),
                            "path": req.path().to_string(),
                            "ip": addr.ip().to_string(),
                            "is_ipv4": addr.is_ipv4(),
                            "is_ipv6": addr.is_ipv6(),
                        });

                        let (tx, rx) = channel();
                        requests_map_inner_clone
                            .lock()
                            .unwrap()
                            .insert(trace_id.clone(), tx);

                        let _ = speaker_clone.send(Response {
                            payload: Ok(Some(payload)),
                            attach: attach.clone(),
                            origin: id,
                            trace_id: trace_id.clone(),
                        });

                        let response: ContentResponse = rx.recv().unwrap();

                        let status_code = if let Some(status_code) = response.status_code {
                            StatusCode::from_u16(status_code).unwrap()
                        } else {
                            StatusCode::from_u16(default_status_code).unwrap()
                        };

                        let mut http_response = HttpResponse::build(status_code);

                        if let Some(headers) = response.headers {
                            for (key, val) in headers {
                                if let Some(val_str) = val.as_str() {
                                    http_response.header(&key, val_str);
                                } else {
                                    log::error!("Could not pass header value to string: {:?}", val);
                                }
                            }
                        }

                        if let Some(body) = response.body {
                            http_response.body(body)
                        } else {
                            http_response.body("".to_string())
                        }
                    },
                )),
            );
        }

        App::new().many_services(services)
    })
    .bind(&address)
    .unwrap()
    .run();

    for request_step in listener {
        if let Ok(mut map) = requests_map.lock() {
            if let Some(sender) = map.get(&request_step.trace_id) {
                trace!("Total request waiting: {}", &map.len());

                let content_response = match request_step.payload {
                    Ok(payload) => {
                        if let Some(payload) = payload {
                            ContentResponse::from_value(payload)
                        } else {
                            log::warn!("The payload from the previous step returned empty");
                            ContentResponse::default()
                        }
                    }
                    Err(err) => {
                        log::error!(
                            "The payload from the previous step returned an error: {:?}",
                            err
                        );

                        ContentResponse {
                            status_code: Some(500),
                            body: None,
                            headers: None,
                        }
                    }
                };

                sender.send(content_response).unwrap();
                map.remove(&request_step.trace_id);
            } else {
                log::error!("Sender not found.")
            }
        } else {
            log::error!("Map error.")
        }
    }

    let _ = sys.run();
}

create_module_raw!(http_server);

#[cfg(test)]
mod tests {
    use super::*;
    use carbon_core::modules::*;
    use reqwest::{Body, IntoUrl};
    use std::thread;

    #[macro_export]
    macro_rules! create_test {
        ($params:tt) => {
            create_test!($params, |payload: Value| { payload })
        };
        ($params:tt, $handler:expr) => {
            let config = Config {
                reference: "test".to_string(),
                params: Some(json!($params)),
                producer: None,
                default_attach: None,
            };

            run_module_raw!(crate::http_server, config, tx, rx);

            thread::spawn(move || {
                let request = rx.recv().unwrap();
                let payload = $handler(request.payload.unwrap().unwrap());

                let _ = tx.send(Request {
                    origin: 1,
                    payload: Ok(Some(payload)),
                    trace_id: request.trace_id,
                });
            });
        };
    }

    fn format_result(text: &str) -> String {
        format!(r#""{}""#, text)
    }

    async fn post<U, B>(url: U, body: B) -> String
    where
        U: IntoUrl,
        B: Into<Body>,
    {
        let client = reqwest::Client::new();
        client
            .post(url)
            .body(body)
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap()
    }

    async fn post_raw<U, B>(url: U, body: B) -> reqwest::Response
    where
        U: IntoUrl,
        B: Into<Body>,
    {
        let client = reqwest::Client::new();
        client.post(url).body(body).send().await.unwrap()
    }

    async fn get_raw<U>(url: U) -> reqwest::Response
    where
        U: IntoUrl,
    {
        let client = reqwest::Client::new();
        client.get(url).send().await.unwrap()
    }

    #[actix_rt::test]
    async fn test_index_ok() {
        create_test!(
            {
                "path": "/my-path/{name}/{other}",
                "address": "127.0.0.1:9301"
            },
            |mut payload: Value| {
                payload["body"] = Value::from(format!("my: {}", &payload["body"].as_str().unwrap()));
                payload
            }
        );

        let body = post(
            "http://127.0.0.1:9301/my-path/foo/fux?item=1&case=bar",
            "foo-baz-fux-bar",
        )
        .await;
        assert_eq!(body, format_result("my: foo-baz-fux-bar"));
    }

    #[actix_rt::test]
    async fn test_port() {
        create_test!(
            {
                "path": "/",
                "port": 9302
            },
            |mut payload: Value| {
                payload["body"] = Value::from("Ok");
                payload
            }
        );

        let body = post("http://127.0.0.1:9302", "").await;
        assert_eq!(body, format_result("Ok"));
    }

    #[actix_rt::test]
    async fn test_custom_status_code() {
        create_test!(
            {
                "path": "/",
                "port": 9303
            },
            |mut payload: Value| {
                payload["status_code"] = Value::from(203i64);
                payload
            }
        );

        let res = post_raw("http://127.0.0.1:9303", "").await;
        assert_eq!(res.status().as_u16(), 203u16);
    }

    #[actix_rt::test]
    async fn test_method_not_found() {
        create_test!(
            {
                "path": "/",
                "method": "PUT",
                "port": 9304
            }
        );

        let res = post_raw("http://127.0.0.1:9304", "").await;
        assert_eq!(res.status().as_u16(), 404u16);
    }

    #[actix_rt::test]
    async fn test_method_any() {
        create_test!(
            {
                "path": "/",
                "method": "ANY",
                "port": 9305
            }
        );

        let res = post_raw("http://127.0.0.1:9305", "").await;
        assert_eq!(res.status().as_u16(), 200u16);
    }

    #[actix_rt::test]
    async fn test_multiple_requests() {
        let config = Config {
            reference: "test".to_string(),
            params: Some(json!({
                "path": "/",
                "method": "ANY",
                "port": 9306
            })),
            producer: None,
            default_attach: None,
        };

        run_module_raw!(crate::http_server, config, tx, rx);

        thread::spawn(move || {
            for request in rx {
                let mut payload = request.payload.unwrap().unwrap();
                payload["body"] = Value::from(request.trace_id.to_string());

                let _ = tx.send(Request {
                    origin: 1,
                    payload: Ok(Some(payload)),
                    trace_id: request.trace_id,
                });
            }
        });

        let mut children = vec![];

        for _ in 1..1000 {
            children.push(post_raw("http://127.0.0.1:9306", "").await);
        }

        for res in children {
            assert_eq!(res.status().as_u16(), 200u16);
        }
    }

    #[actix_rt::test]
    async fn test_multiple_paths() {
        let config = Config {
            reference: "test".to_string(),
            params: Some(json!({
                "routes": [
                    {
                        "path": "/foo/fux",
                        "default_status_code": 203,
                        "method": "POST"
                    },
                    {
                        "path": "/bar/foo",
                        "default_status_code": 202,
                        "method": "POST"
                    },
                    {
                        "path": "/bar/xxx",
                        "default_status_code": 400,
                        "method": "GET"
                    }
                ],
                "path": "/",
                "method": "ANY",
                "port": 9307
            })),
            producer: None,
            default_attach: None,
        };

        run_module_raw!(crate::http_server, config, tx, rx);

        thread::spawn(move || {
            for request in rx {
                let payload = request.payload.unwrap().unwrap();
                let _ = tx.send(Request {
                    origin: 1,
                    payload: Ok(Some(payload)),
                    trace_id: request.trace_id,
                });
            }
        });

        assert_eq!(
            post_raw("http://127.0.0.1:9307/foo/fux", "")
                .await
                .status()
                .as_u16(),
            203u16
        );
        assert_eq!(
            post_raw("http://127.0.0.1:9307/bar/foo", "")
                .await
                .status()
                .as_u16(),
            202u16
        );
        assert_eq!(
            get_raw("http://127.0.0.1:9307/bar/xxx")
                .await
                .status()
                .as_u16(),
            400u16
        );
    }

    #[actix_rt::test]
    async fn test_ip() {
        let config = Config {
            reference: "test".to_string(),
            params: Some(json!({
                "path": "/",
                "method": "ANY",
                "port": 9308
            })),
            producer: None,
            default_attach: None,
        };

        run_module_raw!(crate::http_server, config, tx, rx);

        thread::spawn(move || {
            for request in rx {
                let mut payload = request.payload.unwrap().unwrap();
                payload["body"] = Value::from(payload.clone());

                let _ = tx.send(Request {
                    origin: 1,
                    payload: Ok(Some(payload)),
                    trace_id: request.trace_id,
                });
            }
        });

        let response = post_raw("http://127.0.0.1:9308", "").await;
        let de_body = response.text().await.unwrap();
        let body: Value = serde_json::from_str(&de_body).unwrap();

        assert_eq!(body.get("ip").unwrap().as_str().unwrap(), "127.0.0.1");
    }
}
