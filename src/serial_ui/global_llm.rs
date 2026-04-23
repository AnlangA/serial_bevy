use bevy::prelude::*;

use crate::serial::ai::send_ai_chat;
use crate::serial::data::AiResponse;
use crate::serial::discovery::Runtime;
use crate::serial::llm::LlmMessage;

use super::config::PanelWidths;

/// Runtime-only state for the global LLM panel.
#[derive(Resource, Default)]
pub struct GlobalLlmState {
    /// Whether to show the "missing API key" popup warning.
    pub show_key_missing_popup: bool,
    /// Global LLM messages (kept when no serial port is selected).
    pub messages: Vec<LlmMessage>,
    /// Global LLM input buffer.
    pub input_buffer: String,
    /// Global LLM is currently processing a request.
    pub is_processing: bool,
    /// Global LLM request already dispatched (prevents duplicate spawns).
    pub request_in_flight: bool,
}

/// A dedicated channel for global LLM AI responses, completely separate from
/// `AiChannel` which is used by per-port LLM requests.
#[derive(Resource)]
pub struct GlobalLlmResponse {
    pub tx: std::sync::Mutex<std::sync::mpsc::Sender<AiResponse>>,
    pub rx: std::sync::Mutex<std::sync::mpsc::Receiver<AiResponse>>,
}

impl GlobalLlmResponse {
    #[must_use]
    pub fn init() -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        Self {
            tx: std::sync::Mutex::new(tx),
            rx: std::sync::Mutex::new(rx),
        }
    }
}

/// Dispatches AI requests for the global LLM panel when no serial port is selected.
pub fn process_global_llm_requests(
    runtime: Res<Runtime>,
    global_response: Res<GlobalLlmResponse>,
    panel_widths: Res<PanelWidths>,
    mut global_state: ResMut<GlobalLlmState>,
) {
    if panel_widths.llm_key.is_empty() {
        return;
    }
    let state = &mut *global_state;
    if !state.is_processing || state.request_in_flight {
        return;
    }

    let messages = state.messages.clone();
    let last_is_user = messages.last().map(|m| m.role == "user").unwrap_or(false);
    if !last_is_user {
        return;
    }

    state.request_in_flight = true;

    let model = panel_widths.llm_model.clone();
    let key = panel_widths.llm_key.clone();
    let with_coding_plan = panel_widths.llm_with_coding_plan;

    let tx = global_response
        .tx
        .lock()
        .expect("GlobalLlmResponse tx poisoned")
        .clone();

    runtime.spawn(async move {
        let result = send_ai_chat(&model, key, with_coding_plan, messages).await;
        let (content, is_error) = match result {
            Ok(c) => (c, false),
            Err(c) => (c, true),
        };
        let _ = tx.send(AiResponse {
            port_name: String::new(),
            content,
            is_error,
        });
    });
}

/// Receives completed global LLM responses and appends them into runtime state.
pub fn receive_global_llm_responses(
    global_response: Res<GlobalLlmResponse>,
    mut global_state: ResMut<GlobalLlmState>,
) {
    while let Ok(response) = global_response
        .rx
        .lock()
        .expect("GlobalLlmResponse rx poisoned")
        .try_recv()
    {
        global_state.is_processing = false;
        global_state.request_in_flight = false;

        if response.is_error {
            global_state.messages.push(LlmMessage::assistant(format!(
                "Error: {}",
                response.content
            )));
        } else {
            global_state
                .messages
                .push(LlmMessage::assistant(&response.content));
        }
    }
}
