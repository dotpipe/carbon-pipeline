#[macro_use]
extern crate carbon_core;

use carbon_core::modules::{Config, Listener, Return, TraceId};

fn mock<F: Fn(Return)>(listener: Listener, send: F, config: Config) {
    match config.producer {
        Some(active) if active => {
            let local_trace = TraceId::new();

            send(Return {
                payload: Ok(config.params),
                attach: config.default_attach.clone(),
                trace_id: local_trace.id,
            })
        }
        _ => {}
    };

    for request in listener {
        send(Return {
            payload: request.payload,
            attach: config.default_attach.clone(),
            trace_id: request.trace_id,
        });
    }
}

create_module!(mock);

#[cfg(test)]
mod tests {
    use carbon_core::modules::*;

    #[test]
    fn with_producer() {
        let config = Config {
            reference: "test".parse().unwrap(),
            params: None,
            producer: Some(true),
            default_attach: None,
        };
        create_module_assert_eq!(crate::mock, config, Ok(None), Ok(None), false);
    }

    #[test]
    fn without_producer() {
        let config = Config {
            reference: "test".parse().unwrap(),
            params: None,
            producer: Some(true),
            default_attach: None,
        };
        create_module_assert_eq!(crate::mock, config, Ok(None), Ok(None), true);
    }
}
