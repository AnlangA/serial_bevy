//! # AI Module
//!
//! LLM request orchestration and response handling.

use super::data::{AiChannel, AiResponse};
use super::{LlmMessage, Runtime, Serials};
use bevy::prelude::*;

/// Sends an AI chat request using zai-rs.
async fn send_ai_chat(
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

    let resp: ChatCompletionResponse = match model {
        "glm-4.7" => {
            let mut c = ChatCompletion::new(GLM4_7 {}, first, key);
            for m in chat_msgs {
                c = c.add_messages(m);
            }
            if with_coding_plan {
                c = c.with_coding_plan();
            }
            c.send().await.map_err(|e| e.to_string())?
        }
        "glm-4.6" => {
            let mut c = ChatCompletion::new(GLM4_6 {}, first, key);
            for m in chat_msgs {
                c = c.add_messages(m);
            }
            if with_coding_plan {
                c = c.with_coding_plan();
            }
            c.send().await.map_err(|e| e.to_string())?
        }
        "glm-4.5" => {
            let mut c = ChatCompletion::new(GLM4_5 {}, first, key);
            for m in chat_msgs {
                c = c.add_messages(m);
            }
            if with_coding_plan {
                c = c.with_coding_plan();
            }
            c.send().await.map_err(|e| e.to_string())?
        }
        "glm-4.5-flash" => {
            let mut c = ChatCompletion::new(GLM4_5_flash {}, first, key);
            for m in chat_msgs {
                c = c.add_messages(m);
            }
            if with_coding_plan {
                c = c.with_coding_plan();
            }
            c.send().await.map_err(|e| e.to_string())?
        }
        "glm-4.5-air" => {
            let mut c = ChatCompletion::new(GLM4_5_air {}, first, key);
            for m in chat_msgs {
                c = c.add_messages(m);
            }
            if with_coding_plan {
                c = c.with_coding_plan();
            }
            c.send().await.map_err(|e| e.to_string())?
        }
        "glm-4.5-X" => {
            let mut c = ChatCompletion::new(GLM4_5_x {}, first, key);
            for m in chat_msgs {
                c = c.add_messages(m);
            }
            if with_coding_plan {
                c = c.with_coding_plan();
            }
            c.send().await.map_err(|e| e.to_string())?
        }
        "glm-4.5-airx" => {
            let mut c = ChatCompletion::new(GLM4_5_airx {}, first, key);
            for m in chat_msgs {
                c = c.add_messages(m);
            }
            if with_coding_plan {
                c = c.with_coding_plan();
            }
            c.send().await.map_err(|e| e.to_string())?
        }
        _ => return Err(format!("Unknown model: {model}")),
    };

    let text = resp
        .choices()
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.message().content())
        .and_then(|content| content.as_str().map(ToOwned::to_owned))
        .unwrap_or_default();

    Ok(text)
}

/// System: processes pending AI chat requests.
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
        if !llm.enable
            || !llm.is_processing
            || llm.request_in_flight
            || app_config.llm_key.is_empty()
            || app_config.llm_model.is_empty()
        {
            continue;
        }

        let messages = llm.messages.clone();
        let should_send = messages.last().map(|m| m.role == "user").unwrap_or(false);
        if !should_send {
            continue;
        }

        llm.request_in_flight = true;
        let model = app_config.llm_model.clone();
        let key = app_config.llm_key.clone();
        let with_coding_plan = app_config.llm_with_coding_plan;
        let tx = ai_channel
            .tx
            .lock()
            .expect("AI channel tx poisoned")
            .clone();

        runtime.spawn(async move {
            let result = send_ai_chat(&model, key, with_coding_plan, messages).await;
            let payload = match result {
                Ok(content) => AiResponse {
                    port_name,
                    content,
                    is_error: false,
                },
                Err(content) => AiResponse {
                    port_name,
                    content,
                    is_error: true,
                },
            };
            let _ = tx.send(payload);
        });
    }
}

/// System: receives AI chat responses and updates serial state.
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
