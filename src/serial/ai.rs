//! # AI Module
//!
//! LLM request orchestration and response handling for serial port AI features.

use bevy::prelude::*;

use super::Serials;
use super::data::{AiChannel, AiResponse};
use super::discovery::Runtime;
use super::llm::LlmMessage;

/// Sends an AI chat request using zai-rs.
pub async fn send_ai_chat(
    model: &str,
    key: String,
    with_coding_plan: bool,
    messages: Vec<LlmMessage>,
) -> Result<String, String> {
    use zai_rs::model::{chat_base_response::ChatCompletionResponse, *};

    let mut chat_msgs: Vec<TextMessage> = messages
        .iter()
        .map(|m| match m.role.as_str() {
            "user" => TextMessage::user(&m.content),
            "assistant" => TextMessage::assistant(&m.content),
            _ => TextMessage::user(&m.content),
        })
        .collect();

    if chat_msgs.is_empty() {
        return Err("No messages to send".to_string());
    }

    let first = chat_msgs.remove(0);

    /// Macro to dispatch to a specific GLM model type, reducing repetitive match arms.
    macro_rules! glm_chat {
        ($model:ident, $first:ident, $key:ident, $chat_msgs:ident, $with_coding_plan:ident) => {{
            let mut c = ChatCompletion::new($model {}, $first, $key);
            for m in $chat_msgs {
                c = c.add_messages(m);
            }
            if $with_coding_plan {
                c = c.with_coding_plan();
            }
            c.send().await.map_err(|e| e.to_string())?
        }};
    }

    let resp: ChatCompletionResponse = match model {
        "glm-5.1" => glm_chat!(GLM5_1, first, key, chat_msgs, with_coding_plan),
        "glm-5" => glm_chat!(GLM5, first, key, chat_msgs, with_coding_plan),
        "glm-5-turbo" => glm_chat!(GLM5_turbo, first, key, chat_msgs, with_coding_plan),
        "glm-4.7" => glm_chat!(GLM4_7, first, key, chat_msgs, with_coding_plan),
        "glm-4.7-flash" => glm_chat!(GLM4_7_flash, first, key, chat_msgs, with_coding_plan),
        "glm-4.7-flashx" => glm_chat!(GLM4_7_flashx, first, key, chat_msgs, with_coding_plan),
        "glm-4.6" => glm_chat!(GLM4_6, first, key, chat_msgs, with_coding_plan),
        "glm-4.5" => glm_chat!(GLM4_5, first, key, chat_msgs, with_coding_plan),
        "glm-4.5-flash" => glm_chat!(GLM4_5_flash, first, key, chat_msgs, with_coding_plan),
        "glm-4.5-air" => glm_chat!(GLM4_5_air, first, key, chat_msgs, with_coding_plan),
        "glm-4.5-X" => glm_chat!(GLM4_5_x, first, key, chat_msgs, with_coding_plan),
        "glm-4.5-airx" => glm_chat!(GLM4_5_airx, first, key, chat_msgs, with_coding_plan),
        _ => return Err(format!("Unknown model: {model}")),
    };

    let text = resp
        .choices()
        .and_then(|cs| cs.first())
        .and_then(|c| c.message().content())
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_default();

    Ok(text)
}

/// System: processes pending AI chat requests.
///
/// This system runs every frame and checks if there is a pending AI chat request
/// that needs to be sent. It ensures:
/// - LLM features are enabled for the serial port
/// - A request is currently being processed (user clicked send)
/// - No request is already in flight
/// - The API key and model are configured
/// - The last message is from the user (indicating we need to respond)
pub fn process_ai_requests(
    mut serials: Query<&mut Serials>,
    runtime: Res<Runtime>,
    ai_channel: Res<AiChannel>,
    app_config: Res<crate::serial_ui::PanelWidths>,
) {
    let Ok(mut serials) = serials.single_mut() else {
        return;
    };

    for serial in &mut serials.serial {
        let Ok(mut serial) = serial.lock() else {
            continue;
        };

        let port_name = serial.set.port_name.clone();
        let llm = serial.llm();
        if !llm.enable || !llm.is_processing || app_config.llm_key.is_empty() {
            continue;
        }
        // Prevent spawning duplicate requests every frame while waiting for response
        if llm.request_in_flight {
            continue;
        }

        // Take the messages to send
        let messages = llm.messages.clone();
        let model = app_config.llm_model.clone();
        let key = app_config.llm_key.clone();
        let with_coding_plan = app_config.llm_with_coding_plan;

        // Check if the last message is from user (we need to respond)
        let should_send = messages.last().map(|m| m.role == "user").unwrap_or(false);
        if !should_send {
            continue;
        }

        // Mark request as dispatched so we don't spawn again next frame
        llm.request_in_flight = true;
        let tx = ai_channel
            .tx
            .lock()
            .expect("AI channel tx poisoned")
            .clone();

        // Spawn async task
        runtime.spawn(async move {
            let result = send_ai_chat(&model, key, with_coding_plan, messages).await;

            match result {
                Ok(content) => {
                    let _ = tx.send(AiResponse {
                        port_name,
                        content,
                        is_error: false,
                    });
                }
                Err(content) => {
                    let _ = tx.send(AiResponse {
                        port_name,
                        content,
                        is_error: true,
                    });
                }
            }
        });
    }
}

/// System: receives AI chat responses and updates serial state.
///
/// This system runs every frame and checks for incoming AI chat responses.
/// When a response is received, it updates the corresponding serial port's
/// LLM configuration with the assistant's message.
pub fn receive_ai_responses(mut serials: Query<&mut Serials>, ai_channel: Res<AiChannel>) {
    let Ok(mut serials) = serials.single_mut() else {
        return;
    };

    while let Ok(response) = ai_channel
        .rx
        .lock()
        .expect("AI channel rx poisoned")
        .try_recv()
    {
        for serial in &mut serials.serial {
            let Ok(mut serial) = serial.lock() else {
                continue;
            };

            if serial.set.port_name != response.port_name {
                continue;
            }

            serial.llm().is_processing = false;
            serial.llm().request_in_flight = false;

            if response.is_error {
                serial
                    .llm()
                    .add_assistant_message(&format!("Error: {}", response.content));
            } else {
                serial.llm().add_assistant_message(&response.content);
            }
            break;
        }
    }
}
