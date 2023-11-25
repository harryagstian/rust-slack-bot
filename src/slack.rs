use std::{collections::HashMap, net::TcpStream};

use crate::{
    config::{SlackConfig, SlackToken},
    executor::Executor,
};
use log::info;
use reqwest::header::CONTENT_TYPE;
use serde::{Deserialize, Serialize};
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
    config: SlackConfig,
    websocket_url: String,
}

impl Slack {
    pub async fn new(config: SlackConfig) -> Result<Self, reqwest::Error> {
        let websocket_url =
            Self::get_websocket_address(config.token.get(&SlackToken::Websocket).unwrap()).await?;

        Ok(Slack {
            config,
            websocket_url,
        })
    }

    async fn get_websocket_address(token: &String) -> Result<String, reqwest::Error> {
        let url = "https://slack.com/api/apps.connections.open";
        let res = reqwest::Client::new()
            .post(url)
            .bearer_auth(token)
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .send()
            .await?;

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

    pub fn listen_websocket(&self, executors: Vec<Executor>) -> Result<(), tungstenite::Error> {
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
            let message = from_str::<SlackWebsocketMessage>(raw_text)
                .expect(format!("Unexpected message format. Raw message: {}", &raw_text).as_str());

            match message {
                SlackWebsocketMessage::NormalMessage {
                    payload,
                    envelope_id,
                    ..
                } => match payload.event {
                    SlackWebsocketMessagePayloadEvent::AppMention { text, .. }
                    | SlackWebsocketMessagePayloadEvent::ChannelMessageSent { text, .. } => {
                        info!("Received channel message [{}]: {:?}", envelope_id, text);
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
                    _ => todo!(),
                },
                _ => info!("Received: {:?}", message),
            }
        }
        Ok(())
        // socket.close(None);
    }
}
