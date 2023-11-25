use std::error::Error;

use log::{info, debug};
use rust_slack_bot::config::Config;
use rust_slack_bot::logger::Logger;
use rust_slack_bot::slack::Slack;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    Logger::init();
    let config = Config::new()?;

    let slack = Slack::new(config.slack.clone()).await?;

    info!("{:?}", &config);
    info!("{:?}", &slack);

    slack.listen_websocket(config.executors);

    Ok(())
}
