use std::{collections::HashMap, net::TcpStream};

use crate::{config::SlackConfig, executor::Executors};
use log::{error, info};
use reqwest::header::CONTENT_TYPE;
use serde::Deserialize;
use serde_json::{from_str, json, Value};
use tungstenite::{connect, stream::MaybeTlsStream, Message, WebSocket};

#[derive(Deserialize, Debug, Clone)]
struct SlackHTTPWebsocketUrlResponse {
    ok: bool,
    url: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
enum SlackWebsocketMessage {
    NormalMessage {
        envelope_id: String,
        payload: SlackWebsocketMessagePayload,
        r#type: String,
        accepts_response_payload: bool,
        retry_attempt: u16,
        retry_reason: String,
    },
    HelloMessage {
        r#type: String,
        num_connections: u8,
        debug_info: HashMap<String, Value>,
        connection_info: HashMap<String, String>,
    },
}

#[derive(Deserialize, Debug, Clone)]
struct SlackWebsocketMessagePayload {
    r#type: String,
    event_id: String,
    event_time: i64,
    event: SlackWebsocketMessagePayloadEvent,
    #[serde(flatten)]
    other: HashMap<String, Value>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
enum SlackWebsocketMessagePayloadEvent {
    AppMention {
        client_msg_id: String,
        r#type: String,
        text: String,
        user: String,
        ts: String,
        team: String,
        thread_ts: String,
        parent_user_id: String,
        channel: String,
        event_ts: String,
        #[serde(flatten)]
        other: HashMap<String, Value>,
    },
    ThreadReply {
        bot_id: Option<String>,
        r#type: String,
        text: String,
        user: String,
        ts: String,
        app_id: String,
        team: String,
        thread_ts: String,
        parent_user_id: String,
        channel: String,
        event_ts: String,
        channel_type: String,
        #[serde(flatten)]
        other: HashMap<String, Value>,
    },
    ChannelMessageSent {
        client_msg_id: String,
        r#type: String,
        text: String,
        user: String,
        ts: String,
        team: String,
        channel: String,
        event_ts: String,
        channel_type: String,
        #[serde(flatten)]
        other: HashMap<String, Value>,
    },
    ChannelMessageDeleted {
        r#type: String,
        subtype: String,
        previous_message: Value,
        channel: String,
        hidden: bool,
        deleted_ts: String,
        event_ts: String,
        ts: String,
        channel_type: String,
        #[serde(flatten)]
        other: HashMap<String, Value>,
    },
    ReactionUpdated {
        r#type: String,
        user: String,
        reaction: String,
        item: ReactionItem,
        item_user: String,
        event_ts: String,
    },
}

#[derive(Deserialize, Debug, Clone)]
struct ReactionItem {
    r#type: String,
    channel: String,
    ts: String,
}

#[derive(Debug)]
pub struct Slack {
    request_client: reqwest::Client,
    config: SlackConfig,
    websocket_url: String,
}

impl Slack {
    pub async fn new(config: SlackConfig) -> Result<Self, reqwest::Error> {
        let websocket_url = Self::get_websocket_address(&config.secret.app_token).await?;

        Ok(Slack {
            request_client: reqwest::Client::new(),
            config,
            websocket_url,
        })
    }

    fn create_request<U: reqwest::IntoUrl>(
        &self,
        method: reqwest::Method,
        url: U,
    ) -> reqwest::RequestBuilder {
        self.request_client
            .request(method, url)
            .bearer_auth(&self.config.secret.bot_token) // TODO: do we need to consider using app_token here?
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
    }

    async fn get_websocket_address(secret: &str) -> Result<String, reqwest::Error> {
        let url = "https://slack.com/api/apps.connections.open";
        let res = reqwest::Client::new()
            .post(url)
            .bearer_auth(secret)
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .send()
            .await?;

        // TODO: handle decoding error
        // TODO: handle !res.status().is_success()
        let data = res.json::<SlackHTTPWebsocketUrlResponse>().await?;

        Ok(data.url)
    }

    fn ack_message(
        socket: &mut WebSocket<MaybeTlsStream<TcpStream>>,
        envelope_id: &str,
    ) -> Result<(), tungstenite::Error> {
        let ack_message = json!({"envelope_id": envelope_id}).to_string();

        socket.send(Message::Text(ack_message))?;
        info!("Acked message [{}]", envelope_id);

        Ok(())
    }

    pub async fn listen_websocket(&self, executors: Executors) -> Result<(), tungstenite::Error> {
        let (mut socket, response) = connect(&self.websocket_url)?;

        info!("Connected to the server");
        info!("Response HTTP code: {}", response.status());
        info!("Response contains the following headers:");

        for (ref header, _value) in response.headers() {
            info!("* {}", header);
        }

        loop {
            let raw_message = socket.read().expect("Error reading message");

            if raw_message.is_ping() {
                continue;
            }

            let raw_text = &raw_message.into_text().unwrap();
            let message = match from_str::<SlackWebsocketMessage>(raw_text) {
                Ok(v) => v,
                Err(err) => {
                    error!(
                        "Error: {}.\nProbably unexpected message format. Raw message:\n{}",
                        &err, &raw_text
                    );

                    // ack the message anyway
                    error!("Trying to parse and ack the message regardless");
                    let v = from_str::<Value>(raw_text).unwrap();
                    let envelope_id = v["envelope_id"].as_str().unwrap();
                    let _ = Self::ack_message(&mut socket, envelope_id);
                    continue;
                }
            };

            // panic!("Unexpected message format. Raw message: {}", &raw_text)

            match message {
                SlackWebsocketMessage::NormalMessage {
                    payload,
                    envelope_id,
                    ..
                } => match payload.event {
                    SlackWebsocketMessagePayloadEvent::AppMention { text, ts, .. }
                    | SlackWebsocketMessagePayloadEvent::ChannelMessageSent { text, ts, .. } => {
                        info!("Received channel message [{}]: {:?}", envelope_id, text);
                        // TODO: implement error short circuit here, and add more error type
                        let _ = executors.execute_from_slack_message(&text);

                        self.post_message(&text, Some(&ts)).await;

                        Self::ack_message(&mut socket, &envelope_id)?;
                    }
                    SlackWebsocketMessagePayloadEvent::ReactionUpdated {
                        r#type, reaction, ..
                    } => {
                        info!(
                            "Received reaction updates [{}]: {} - {}",
                            envelope_id, r#type, reaction
                        );
                        Self::ack_message(&mut socket, &envelope_id)?;
                    }
                    SlackWebsocketMessagePayloadEvent::ThreadReply { .. } => {
                        // do nothing for now
                    }
                    _ => {
                        todo!("Unhandled event type: {:?}", &payload.event)
                    }
                },
                _ => info!("Received: {:?}", message),
            }
        }
        Ok(())
        // socket.close(None);
    }

    pub async fn post_message(
        &self,
        _message: &str,
        ts: Option<&str>,
    ) -> Result<(), reqwest::Error> {
        // https://api.slack.com/methods/chat.postMessage
        let url = "https://slack.com/api/chat.postMessage";
        let mut params: HashMap<&str, &str> = HashMap::new();

        // TODO: channel needs to be dynamic
        params.insert("channel", "rust-slack-bot");
        params.insert("text", "Ok! ✅");
        params.insert("icon_emoji", ":sushi:"); // i like sushi, why not?
        if let Some(v) = ts {
            params.insert("thread_ts", v);
        }

        let res = self
            .create_request(reqwest::Method::POST, url)
            .form(&params)
            .send()
            .await?;

        res.error_for_status_ref()?;

        let text = res.json::<Value>().await.unwrap();

        if let Some(x) = text.get("ok") {
            let x = x.as_bool().unwrap();
            if !x {
                // TODO: handle error
                dbg!(&text);
            }
        }
        // TODO: handle HTTP error

        Ok(())
    }
}
