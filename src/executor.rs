use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Executor {
    name: String,
    binary: String,
    command: String,
}

impl Executor {
    pub fn parse_slack_message(message: &str) {
        let vec_message = message.lines().map(|x| x.trim()).collect::<Vec<&str>>();

    }
    pub fn execute_from_slack_message(executors: Vec<Executor>, message: &str) {

    }
}
