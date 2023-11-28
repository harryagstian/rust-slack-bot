use std::collections::HashMap;

use serde::{Serialize, Deserialize};

use crate::executor::Executors;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    pub env: String,
    pub slack: SlackConfig,
    pub executors: Executors,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SlackConfig {
    pub secret: HashMap<SlackSecret, String>
}

#[derive(Hash, Eq, PartialEq, Serialize, Deserialize, Debug, Clone)]
pub enum SlackSecret {
    #[serde(rename = "websocket")]
    Websocket,
    #[serde(rename = "webhook_url")]
    WebhookURL,
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
