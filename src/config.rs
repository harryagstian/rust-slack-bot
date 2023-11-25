use std::collections::HashMap;

use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    pub env: String,
    pub slack: SlackConfig,
    pub executors: Vec<crate::executor::Executor>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SlackConfig {
    pub token: HashMap<SlackToken, String>
}

#[derive(Hash, Eq, PartialEq, Serialize, Deserialize, Debug, Clone)]
pub enum SlackToken {
    #[serde(rename = "websocket")]
    Websocket
}

impl Config {
    pub fn new() -> Result<Self, config::ConfigError> {
        let raw_settings = config::Config::builder()
            .add_source(config::File::with_name("config"))
            .build()?;

            let parsed_settings = raw_settings.try_deserialize::<Config>()?;

        Ok(parsed_settings)
    }
}
