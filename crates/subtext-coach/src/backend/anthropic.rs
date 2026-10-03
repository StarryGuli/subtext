//! Anthropic Messages API：`POST /v1/messages`。

use std::time::Duration;

use serde_json::{Value, json};

use super::{Backend, http};
use crate::{CoachConfig, CoachError};

const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";

const API_VERSION: &str = "2023-06-01";

/// 回复长度上限：三到五个教学点加译文，够用。
const MAX_TOKENS: u32 = 1800;

pub struct Anthropic {
    base_url: String,

    model: String,

    api_key: Option<String>,

    key_env: String,

    timeout: Duration,
}

impl Anthropic {
    pub fn new(config: &CoachConfig) -> Self {
        Self::with_base_url(config, DEFAULT_BASE_URL)
    }

    fn with_base_url(config: &CoachConfig, base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_owned(),
            model: config.anthropic_model.clone(),
            api_key: config.resolve_anthropic_key(),
            key_env: config.anthropic_api_key_env.clone(),
            timeout: config.timeout(),
        }
    }
}

impl Backend for Anthropic {
    fn complete(&self, system: &str, user: &str) -> Result<String, CoachError> {
        let key = self
            .api_key
            .as_deref()
            .ok_or_else(|| CoachError::MissingApiKey(self.key_env.clone()))?;
        let body = json!({
            "model": self.model,
            "max_tokens": MAX_TOKENS,
            "temperature": 0.3,
            "system": system,
            "messages": [{"role": "user", "content": user}],
        });
        let response = http::client(self.timeout)?
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", key)
            .header("anthropic-version", API_VERSION)
            .json(&body)
            .send()
            .map_err(|error| http::map_error(error, self.timeout))?;
        let value: Value = http::check(response)?
            .json()
            .map_err(|error| http::map_error(error, self.timeout))?;
        let text: String = value["content"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|block| block["type"] == "text")
            .filter_map(|block| block["text"].as_str())
            .collect();
        if text.trim().is_empty() {
            return Err(CoachError::EmptyReply);
        }
        Ok(text)
    }

    fn describe(&self) -> String {
        format!("Anthropic · {}", self.model)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::http::test_server;

    fn config(key: Option<&str>) -> CoachConfig {
        CoachConfig {
            anthropic_api_key: key.map(str::to_owned),
            anthropic_api_key_env: "SUBTEXT_TEST_NO_SUCH_ENV".to_owned(),
            timeout_ms: 5000,
            ..CoachConfig::default()
        }
    }

    #[test]
    fn posts_messages_with_key_and_version_headers() {
        let (base, seen) = test_server::once(
            200,
            r#"{"content":[{"type":"text","text":"{\"ok\":"},{"type":"text","text":"1}"}]}"#,
        );
        let backend = Anthropic::with_base_url(&config(Some("sk-ant-test")), &base);
        assert_eq!(backend.complete("SYS", "USER").unwrap(), r#"{"ok":1}"#);
        let seen = seen.join().unwrap();
        let head = seen.head.to_lowercase();
        assert!(head.starts_with("post /v1/messages"));
        assert!(head.contains("x-api-key: sk-ant-test"));
        assert!(head.contains("anthropic-version: 2023-06-01"));
        let body: Value = serde_json::from_str(&seen.body).unwrap();
        assert_eq!(body["system"], "SYS");
        assert_eq!(body["messages"][0]["content"], "USER");
    }

    #[test]
    fn missing_key_and_api_errors() {
        assert!(matches!(
            Anthropic::with_base_url(&config(None), "http://127.0.0.1:1").complete("s", "u"),
            Err(CoachError::MissingApiKey(_))
        ));
        let (base, _seen) = test_server::once(529, r#"{"error":{"type":"overloaded_error"}}"#);
        let error = Anthropic::with_base_url(&config(Some("k")), &base).complete("s", "u").unwrap_err();
        assert!(matches!(error, CoachError::Api { status: 529, .. }));
    }
}
