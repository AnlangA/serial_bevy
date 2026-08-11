//! # LLM Module
//!
//! LLM configuration and message types for AI features.

/// Available text models for AI chat.
pub const TEXT_MODELS: &[(&str, &str)] = &[
    ("glm-5.1", "GLM-5.1"),
    ("glm-5", "GLM-5"),
    ("glm-5-turbo", "GLM-5-Turbo"),
    ("glm-4.7", "GLM-4.7"),
    ("glm-4.7-flash", "GLM-4.7-Flash"),
    ("glm-4.7-flashx", "GLM-4.7-FlashX"),
    ("glm-4.6", "GLM-4.6"),
    ("glm-4.5", "GLM-4.5"),
    ("glm-4.5-flash", "GLM-4.5-Flash"),
    ("glm-4.5-air", "GLM-4.5-Air"),
    ("glm-4.5-X", "GLM-4.5-X"),
    ("glm-4.5-airx", "GLM-4.5-AirX"),
];

/// LLM configuration for AI features (per-serial state).
pub struct LlmConfig {
    /// Whether LLM features are enabled for this serial port.
    pub enable: bool,
    /// Conversation history messages (role, content).
    pub messages: Vec<LlmMessage>,
    /// Current user input buffer.
    pub input_buffer: String,
    /// Whether an AI request is pending (user clicked send).
    pub is_processing: bool,
    /// Whether the request has already been dispatched to async runtime.
    /// Prevents spawning duplicate requests every frame.
    pub request_in_flight: bool,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl LlmConfig {
    /// Creates a new LLM configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            enable: false,
            messages: Vec::new(),
            input_buffer: String::new(),
            is_processing: false,
            request_in_flight: false,
        }
    }

    /// Gets a mutable reference to the enable flag.
    pub const fn enable(&mut self) -> &mut bool {
        &mut self.enable
    }

    /// Adds a user message to the conversation.
    pub fn add_user_message(&mut self, content: &str) {
        self.messages.push(LlmMessage::user(content));
    }

    /// Adds an assistant message to the conversation.
    pub fn add_assistant_message(&mut self, content: &str) {
        self.messages.push(LlmMessage::assistant(content));
    }

    /// Clears the conversation history.
    pub fn clear_messages(&mut self) {
        self.messages.clear();
    }

    /// Returns true if there are messages.
    #[must_use]
    pub fn has_messages(&self) -> bool {
        !self.messages.is_empty()
    }
}

/// A message in an LLM conversation.
#[derive(Clone, Debug, PartialEq)]
pub struct LlmMessage {
    /// The role (user, assistant, system).
    pub role: String,
    /// The message content.
    pub content: String,
    /// Timestamp when the message was created.
    pub timestamp: String,
}

impl LlmMessage {
    /// Creates a new user message with current timestamp.
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: String::from("user"),
            content: content.into(),
            timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
        }
    }

    /// Creates a new assistant message with current timestamp.
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: String::from("assistant"),
            content: content.into(),
            timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llm_config() {
        let mut config = LlmConfig::new();
        assert!(!*config.enable());
        assert!(config.messages.is_empty());
        assert!(!config.is_processing);
        assert!(!config.request_in_flight);

        config.add_user_message("Hello");
        assert_eq!(config.messages.len(), 1);
        assert_eq!(config.messages[0].role, "user");

        config.add_assistant_message("Hi there");
        assert_eq!(config.messages.len(), 2);
        assert_eq!(config.messages[1].role, "assistant");

        config.clear_messages();
        assert!(config.messages.is_empty());
    }

    #[test]
    fn text_models_include_current_zai_rs_text_models() {
        for model in [
            "glm-5.1",
            "glm-5",
            "glm-5-turbo",
            "glm-4.7",
            "glm-4.7-flash",
            "glm-4.7-flashx",
            "glm-4.6",
            "glm-4.5",
            "glm-4.5-flash",
            "glm-4.5-air",
            "glm-4.5-X",
            "glm-4.5-airx",
        ] {
            assert!(TEXT_MODELS.iter().any(|(id, _)| *id == model));
        }
    }
}
