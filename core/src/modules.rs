extern crate serde_json;

pub use serde_json::json;
use serde_json::{Map, Value};

use std::fmt::Debug;
pub use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;
use std::{any::Any, sync::Arc};

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Config {
    pub reference: String,
    pub params: Option<Value>,
    pub producer: Option<bool>,
    pub default_attach: Option<String>,
}

impl Config {
    pub fn get_params_object(self) -> Map<String, Value> {
        match self.params {
            Some(params) => match params.as_object() {
                Some(params) => params.clone(),
                None => Map::new(),
            },
            None => Map::new(),
        }
    }

    pub fn get_params_string(self) -> String {
        match self.params {
            Some(params) => match params.as_str() {
                Some(params) => params.to_string(),
                None => String::default(),
            },
            None => String::default(),
        }
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct Request {
    pub origin: ID,
    pub payload: Result<Option<Value>, Option<Value>>,
    pub trace_id: ID,
}

pub type Payload = Result<Option<Value>, Option<Value>>;
pub type Listener = Receiver<Request>;
pub type Speaker = Sender<Response>;

#[derive(Debug)]
#[allow(dead_code)]
pub struct Response {
    pub payload: Result<Option<Value>, Option<Value>>,
    pub attach: Option<String>,
    pub origin: ID,
    pub trace_id: ID,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct Return {
    pub payload: Result<Option<Value>, Option<Value>>,
    pub attach: Option<String>,
    pub trace_id: ID,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct ModuleContact {
    pub tx: Sender<Request>,
    pub id: ID,
}

#[derive(Debug)]
pub struct ResponseComplete {
    pub origin: String,
    pub payload: Result<Option<Value>, Option<Value>>,
    pub origin_params: Option<Value>,
}

pub type ID = u32;

#[allow(dead_code)]
pub trait Module: Any + Send {
    fn requests(&self, id: ID, request: Sender<ModuleContact>) -> Listener {
        let (tx_req, rx_req): (Sender<Request>, Listener) = channel();
        request.send(ModuleContact { tx: tx_req, id }).unwrap();
        rx_req
    }
    fn start(
        &self,
        _id: ID,
        _request: Sender<ModuleContact>,
        _response: Sender<Response>,
        _config: Config,
    ) {
    }
}

use uuid::Uuid;

pub fn get_trace() -> String {
    Uuid::new_v4().to_string()
}

pub struct TraceId {
    pub id: ID,
}

impl TraceId {
    pub fn new() -> TraceId {
        TraceId {
            id: ID::min_value(),
        }
    }

    pub fn global() -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(TraceId::new()))
    }

    pub fn get_trace(&mut self) -> ID {
        self.id = self.id + 1;

        if self.id > ID::max_value() {
            self.id = ID::min_value();
        }

        self.id.clone()
    }
}

#[macro_export]
macro_rules! declare_module {
    ($module_type:ty, $constructor:path) => {
        #[no_mangle]
        pub extern "C" fn _Module() -> *mut $crate::modules::Module {
            let constructor: fn() -> $module_type = $constructor;
            let object = constructor();
            let boxed: Box<$crate::modules::Module> = Box::new(object);
            Box::into_raw(boxed)
        }
    };
}

#[macro_export]
macro_rules! create_module_raw {
    ($handler:ident) => {
        #[derive(Debug, Default)]
        pub struct Custom {}

        impl $crate::modules::Module for Custom {
            fn start(
                &self,
                module_id: $crate::modules::ID,
                req: $crate::modules::Sender<$crate::modules::ModuleContact>,
                res: $crate::modules::Sender<$crate::modules::Response>,
                config: $crate::modules::Config,
            ) {
                $handler(module_id, self.requests(module_id, req), res, config)
            }
        }

        declare_module!(Custom, Custom::default);
    };
}

#[macro_export]
macro_rules! create_module_producer {
    ($handler:ident) => {
        #[derive(Debug, Default)]
        pub struct Custom {}

        impl $crate::modules::Module for Custom {
            fn start(
                &self,
                module_id: $crate::modules::ID,
                req: $crate::modules::Sender<$crate::modules::ModuleContact>,
                res: $crate::modules::Sender<$crate::modules::Response>,
                config: $crate::modules::Config,
            ) {
                let trace =
                    std::sync::Arc::new(std::sync::Mutex::new($crate::modules::TraceId::new()));

                $handler(
                    self.requests(module_id, req),
                    |result: $crate::modules::Return| {
                        res.send($crate::modules::Response {
                            payload: result.payload,
                            attach: result.attach,
                            origin: module_id,
                            trace_id: trace.lock().unwrap().get_trace(),
                        })
                        .unwrap();
                    },
                    config,
                )
            }
        }

        declare_module!(Custom, Custom::default);
    };
}

#[macro_export]
macro_rules! create_module {
    ($handler:ident) => {
        #[derive(Debug, Default)]
        pub struct Custom {}

        impl $crate::modules::Module for Custom {
            fn start(
                &self,
                module_id: $crate::modules::ID,
                req: $crate::modules::Sender<$crate::modules::ModuleContact>,
                res: $crate::modules::Sender<$crate::modules::Response>,
                config: $crate::modules::Config,
            ) {
                $handler(
                    self.requests(module_id, req),
                    |result: $crate::modules::Return| {
                        res.send($crate::modules::Response {
                            payload: result.payload,
                            attach: result.attach,
                            origin: module_id,
                            trace_id: result.trace_id,
                        })
                        .unwrap();
                    },
                    config,
                )
            }
        }

        declare_module!(Custom, Custom::default);
    };
}

#[macro_export]
macro_rules! create_module_listener {
    ($handler:ident) => {
        #[derive(Debug, Default)]
        pub struct Custom {}

        impl $crate::modules::Module for Custom {
            fn start(
                &self,
                module_id: $crate::modules::ID,
                req: $crate::modules::Sender<$crate::modules::ModuleContact>,
                res: $crate::modules::Sender<$crate::modules::Response>,
                config: $crate::modules::Config,
            ) {
                for request in self.requests(module_id, req) {
                    let result = $handler(request);

                    res.send($crate::modules::Response {
                        payload: result.payload,
                        attach: result.attach,
                        origin: module_id,
                    })
                    .unwrap();
                }
            }
        }

        declare_module!(Custom, Custom::default);
    };
}

#[macro_export]
macro_rules! create_module_assert_eq {
    ($module:expr, $config:expr) => {
        create_module_assert_eq!($module, $config, Ok(None), Ok(None), true);
    };
    ($module:expr, $config:expr, $payload:expr, $compare:expr) => {
        create_module_assert_eq!($module, $config, $payload, $compare, true);
    };
    ($module:expr, $config:expr, $payload:expr, $compare:expr, $producer:expr) => {
        let (tx_res, rx_res): (
            $crate::modules::Sender<$crate::modules::Response>,
            $crate::modules::Receiver<$crate::modules::Response>,
        ) = $crate::modules::channel();
        let (tx_req, rx_req): (
            $crate::modules::Sender<$crate::modules::Request>,
            $crate::modules::Listener,
        ) = $crate::modules::channel();

        std::thread::spawn(move || {
            $module(
                rx_req,
                |result: $crate::modules::Return| {
                    tx_res
                        .send($crate::modules::Response {
                            payload: result.payload,
                            attach: result.attach,
                            origin: 0,
                            trace_id: 0,
                        })
                        .unwrap();
                },
                $config,
            );
        });

        if ($producer) {
            tx_req
                .send($crate::modules::Request {
                    payload: $payload,
                    origin: 0,
                    trace_id: 0,
                })
                .unwrap();
        }

        let left = rx_res.recv().unwrap().payload;

        assert_eq!(left, $compare)
    };
}

#[macro_export]
macro_rules! create_module_assert_eq_attach {
    ($module:expr, $config:expr, $payload:expr, $compare:expr) => {
        let (tx_res, rx_res): (
            $crate::modules::Sender<$crate::modules::Response>,
            $crate::modules::Receiver<$crate::modules::Response>,
        ) = $crate::modules::channel();
        let (tx_req, rx_req): (
            $crate::modules::Sender<$crate::modules::Request>,
            $crate::modules::Listener,
        ) = $crate::modules::channel();

        std::thread::spawn(move || {
            $module(
                rx_req,
                |result: $crate::modules::Return| {
                    tx_res
                        .send($crate::modules::Response {
                            payload: result.payload,
                            attach: result.attach,
                            origin: 0,
                            trace_id: 0,
                        })
                        .unwrap();
                },
                $config,
            );
        });

        tx_req
            .send($crate::modules::Request {
                payload: $payload,
                origin: 0,
                trace_id: 0,
            })
            .unwrap();

        let left = rx_res.recv().unwrap().attach;

        assert_eq!(left, $compare)
    };
}

#[macro_export]
macro_rules! run_module_raw {
    ($module:expr, $config:expr, $tx:ident, $rx:ident) => {
        let ($tx, rreq): (
            $crate::modules::Sender<$crate::modules::Request>,
            $crate::modules::Listener,
        ) = $crate::modules::channel();
        let (tres, $rx): (
            $crate::modules::Sender<$crate::modules::Response>,
            $crate::modules::Receiver<$crate::modules::Response>,
        ) = $crate::modules::channel();

        std::thread::spawn(move || {
            $module(0, rreq, tres, $config);
        });
    };
}
