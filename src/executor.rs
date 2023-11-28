use std::collections::HashMap;

use duct::cmd;
use handlebars::{Handlebars, RenderError, TemplateError};
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, Eq)]
pub struct Executor {
    command: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Executors {
    executors: HashMap<String, Executor>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ExecutorInput {
    name: String,
    input: String,
}

#[derive(Error, Debug, PartialEq, Eq)]
pub enum ParseCodeBlockError {
    #[error("failed to split message '{0}'")]
    SplitFailure(String),
}

#[derive(Error, Debug)]
pub enum ExecutorInputError {
    #[error("failed to extract code block from slack message '{0}'")]
    ExtractCodeBlockError(String),
    #[error(transparent)]
    ParseCodeBlockError(#[from] ParseCodeBlockError),
}

#[derive(Error, Debug)]
pub enum ExecutorExecuteError {
    #[error("no available executors!")]
    NoAvailableExecutors,
    #[error("there is no executor named '{0}'")]
    NoSuchExecutors(String),
    #[error(transparent)]
    ExecutorInputError(#[from] ExecutorInputError),
    #[error(transparent)]
    TemplateError(#[from] TemplateError),
    #[error(transparent)]
    RenderError(#[from] RenderError),
}

impl ExecutorInput {
    pub fn new_from_slack(message: &str) -> Result<Self, ExecutorInputError> {
        let result = Self::extract_code_block_from_slack_message(message);

        let extracted_message = match result {
            Some(v) => v,
            None => {
                return Err(ExecutorInputError::ExtractCodeBlockError(
                    message.to_string(),
                ))
            }
        };

        let executor_input = Self::parse_code_block(extracted_message)?;

        Ok(executor_input)
    }

    pub fn extract_code_block_from_slack_message(message: &str) -> Option<&str> {
        let marker = "```";

        if let Some(start) = message.find(marker) {
            let start = start + marker.len();
            if let Some(end) = message[start..].find(marker) {
                return Some(&message[start..start + end]);
            }
        }

        None
    }

    pub fn parse_code_block(message: &str) -> Result<Self, ParseCodeBlockError> {
        let vec_message = message.lines().map(|x| x.trim()).collect::<Vec<&str>>();
        dbg!(&vec_message);

        let mut result = Self::default();

        for line in vec_message {
            if line.starts_with('#') {
                let raw_line = line.trim_start_matches('#').trim();
                let mut arr = raw_line.split(':').collect::<Vec<&str>>();

                let value = arr
                    .pop()
                    .ok_or(ParseCodeBlockError::SplitFailure(raw_line.to_string()))?;
                let key = arr
                    .pop()
                    .ok_or(ParseCodeBlockError::SplitFailure(raw_line.to_string()))?;

                match key {
                    "executor" => {
                        result.name = value.trim().to_string();
                    }
                    _ => unimplemented!(),
                }
            } else {
                result.input += line;
            }
        }

        Ok(result)
    }
}

impl Executor {
    pub fn prepare_command(&self, input: &str) -> Result<String, ExecutorExecuteError> {
        let mut command_template = Handlebars::new();
        let template_name = "command";
        command_template.register_template_string(template_name, &self.command)?;
        let command = command_template.render(template_name, &json!({ "input": input}))?;

        Ok(command)
    }

    pub fn execute(&self, input: ExecutorInput) -> Result<(), ExecutorExecuteError> {
        let command = self.prepare_command(&input.input)?;
        let stdout = cmd!("/bin/bash", "-c", command);

        dbg!(stdout.read().unwrap());

        Ok(())
    }
}

impl Default for Executors {
    fn default() -> Self {
        Self {
            executors: HashMap::from([(
                "echo".to_string(),
                Executor {
                    command: "echo {{ input }}".to_string(),
                },
            )]),
        }
    }
}

impl Executors {
    pub fn new() -> Result<Self, config::ConfigError> {
        let raw_settings = config::Config::builder()
            .add_source(config::File::with_name("executors"))
            .build()?;

        let parsed_settings = raw_settings.try_deserialize::<Self>()?;

        Ok(parsed_settings)
    }

    pub fn execute_from_slack_message(&self, message: &str) -> Result<(), ExecutorExecuteError> {
        if self.executors.is_empty() {
            return Err(ExecutorExecuteError::NoAvailableExecutors);
        }

        let input = ExecutorInput::new_from_slack(message)?;

        let executor = self
            .executors
            .get(&input.name)
            .ok_or(ExecutorExecuteError::NoSuchExecutors(input.name.clone()))?;

        let _ = executor.execute(input);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_executor_input_parse_message() -> Result<(), ParseCodeBlockError> {
        let text = "# executor: psql\nselect * from status;";
        let result = ExecutorInput::parse_code_block(text)?;
        assert_eq!(
            result,
            ExecutorInput {
                name: "psql".to_string(),
                input: "select * from status;".to_string(),
            }
        );
        Ok(())
    }

    #[test]
    fn test_executor_input_parse_message_error_split_failure() {
        let text = "# executor psql\nselect * from status;";
        let result = ExecutorInput::parse_code_block(text);

        let e = result.unwrap_err();

        assert_eq!(
            e,
            ParseCodeBlockError::SplitFailure("executor psql".to_string())
        );
    }

    // #[test]
    // fn executor_extract_code_block() {
    //     let text =
    //         "```# executor: psql\nselect * from status;```\nsome message";
    //     assert_eq!(
    //         Executor::extract_code_block_from_slack_message(text).unwrap(),
    //         "# executor: psql\nselect * from status;"
    //     );
    // }
}
