#[macro_use]
extern crate carbon_core;
extern crate log;

use carbon_core::log::setup as log_setup;
use carbon_core::modules::{Config, Listener, Response, Speaker, TraceId, ID};
use serde_json::Value;
use std::sync::{Arc, Mutex};
use std::{thread, time};

fn sleep(millis: u64) {
    let ten_millis = time::Duration::from_millis(millis);
    thread::sleep(ten_millis);
}

fn dispatcher(id: ID, listener: Listener, speaker: Speaker, config: Config) {
    log_setup();

    let trace = TraceId::global();
    let attach = config.default_attach.clone();
    let params = config.get_params_object();
    let payload = match params.get("payload") {
        Some(payload) => payload.clone(),
        None => Value::from(""),
    };
    let number_of_dispatch = match params.get("number_of_dispatch") {
        Some(value) => value.as_i64().unwrap_or(1),
        None => 1,
    };
    let spawn_rate = match params.get("spawn_rate") {
        Some(value) => value.as_i64().unwrap_or(1),
        None => 1,
    };
    let spawn_interval = match params.get("spawn_interval") {
        Some(value) => value.as_i64().unwrap_or(1) as u64,
        None => 0,
    };
    let total_dispatch = Arc::new(Mutex::new(0i64));

    log::info!("Spawner start...");

    let trace_th = trace.clone();
    let payload_th = payload.clone();
    let speaker_th = speaker.clone();
    let attach_th = attach.clone();

    thread::spawn(move || loop {
        if total_dispatch.lock().unwrap().ge(&number_of_dispatch) {
            return ();
        }

        for _ in 0..spawn_rate {
            let trace_id = trace_th.lock().unwrap().get_trace();

            if total_dispatch.lock().unwrap().ge(&number_of_dispatch) {
                break;
            }

            speaker_th
                .send(Response {
                    origin: id,
                    trace_id,
                    payload: Ok(Some(payload_th.clone())),
                    attach: attach_th.clone(),
                })
                .unwrap();

            let mut total = total_dispatch.lock().unwrap();
            *total += 1;

            log::info!("Total spawner: {}", total);
        }

        sleep(spawn_interval);
    });

    for _ in listener {
        let trace_id = trace.lock().unwrap().get_trace();

        speaker
            .send(Response {
                origin: id,
                trace_id,
                payload: Ok(Some(payload.clone())),
                attach: attach.clone(),
            })
            .unwrap();
    }
}

create_module_raw!(dispatcher);

#[cfg(test)]
mod tests {
    use carbon_core::modules::*;

    #[test]
    fn test() {
        let config = Config {
            reference: "test".parse().unwrap(),
            params: Some(json!({
                "number_of_dispatch": 10,
                "spawn_rate": 1,
                "spawn_interval": 1000,
            })),
            producer: Some(true),
            default_attach: None,
        };

        run_module_raw!(crate::dispatcher, config, tx, rx);

        let mut limit = 0;

        for response in rx {
            if limit.eq(&10) {
                break;
            }

            limit = limit + 1;

            tx.send(Request {
                origin: 2,
                trace_id: response.trace_id,
                payload: Ok(None),
            })
            .unwrap();
        }
    }
}
