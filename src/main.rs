use std::error::Error;

use log::info;
use rust_slack_bot::config::Config;
use rust_slack_bot::executor::Executors;
use rust_slack_bot::logger::Logger;
use rust_slack_bot::slack::Slack;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    Logger::init();
    let config = Config::load_config()?;
    dbg!(&config);
    let executors = Executors::new()?;
    dbg!(&executors);
    let slack = Slack::new(config.slack.clone()).await?;
    dbg!(&slack);
    info!("{:?}", &config);
    info!("{:?}", &slack);

    slack.listen_websocket(executors).await?;

    Ok(())
}
