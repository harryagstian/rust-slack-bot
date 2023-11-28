

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    pub slack: SlackConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SlackConfig {
    pub secret: SlackSecret,
}

#[derive(Hash, Eq, PartialEq, Serialize, Deserialize, Debug, Clone)]
pub struct SlackSecret {
    pub app_token: String,
    pub bot_token: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            slack: SlackConfig {
                secret: SlackSecret {
                    app_token: "xapp-xxxxx".to_string(),
                    bot_token: "xoxb-xxxxx".to_string(),
                },
            },
        }
    }
}

impl Config {
    pub fn load_config() -> Result<Self, config::ConfigError> {
        let raw_settings = config::Config::builder()
            .add_source(config::File::with_name("config"))
            .build()?;

        let parsed_settings = raw_settings.try_deserialize::<Self>()?;

        Ok(parsed_settings)
    }
}
