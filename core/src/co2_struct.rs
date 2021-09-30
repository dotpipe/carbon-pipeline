use serde_json::Value;

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Step {
    #[serde(rename = "use")]
    pub module: Option<String>,
    pub params: Option<Value>,
    pub payload: Option<Payload>,
    #[serde(rename = "ref")]
    pub reference: Option<String>,
    pub producer: Option<bool>,
    pub attach: Option<String>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Payload {
    pub request: Option<Value>,
    pub response: Option<Value>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Module {
    pub name: String,
    pub source: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Co2 {
    pub version: Option<String>,
    pub modules: Option<Vec<Module>>,
    pub pipeline: Vec<Step>,
}
