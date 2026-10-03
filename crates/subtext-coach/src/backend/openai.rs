//! 自填的 OpenAI 兼容接口：`POST <base>/chat/completions`。

use std::time::Duration;

use serde_json::{Value, json};

use super::{Backend, http};
use crate::{CoachConfig, CoachError};

/// 采样温度：教练要稳，不要花。
const TEMPERATURE: f64 = 0.3;

/// 回复长度上限：三到五个教学点加译文，够用且防止长篇大论。
const MAX_TOKENS: u32 = 1800;

pub struct OpenAi {
    base_url: String,

    model: String,

    api_key: Option<String>,

    key_env: String,

    reasoning_effort: String,

    timeout: Duration,
}

impl OpenAi {
    pub fn new(config: &CoachConfig) -> Self {
        Self {
            base_url: config.openai_base_url.trim_end_matches('/').to_owned(),
            model: config.openai_model.clone(),
            api_key: config.resolve_openai_key(),
            key_env: config.openai_api_key_env.clone(),
            reasoning_effort: config.reasoning_effort.trim().to_owned(),
            timeout: config.timeout(),
        }
    }

    fn request_body(&self, system: &str, user: &str) -> Value {
        let mut body = json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": user},
            ],
            "temperature": TEMPERATURE,
            "max_tokens": MAX_TOKENS,
        });
        if self.reasoning_effort.is_empty() {
            return body;
        }
        if self.is_zhipu() {
            // 智谱不认 reasoning_effort，关思考用 thinking 字段
            if self.reasoning_effort == "none" {
                body["thinking"] = json!({"type": "disabled"});
            }
        } else {
            body["reasoning_effort"] = json!(self.reasoning_effort);
        }
        body
    }

    fn is_zhipu(&self) -> bool {
        reqwest::Url::parse(&self.base_url)
            .ok()
            .and_then(|url| url.host_str().map(str::to_ascii_lowercase))
            .is_some_and(|host| {
                ["bigmodel.cn", "z.ai"]
                    .iter()
                    .any(|domain| host == *domain || host.ends_with(&format!(".{domain}")))
            })
    }
}

impl Backend for OpenAi {
    fn complete(&self, system: &str, user: &str) -> Result<String, CoachError> {
        let key = self
            .api_key
            .as_deref()
            .ok_or_else(|| CoachError::MissingApiKey(self.key_env.clone()))?;
        let response = http::client(self.timeout)?
            .post(format!("{}/chat/completions", self.base_url))
            .bearer_auth(key)
            .json(&self.request_body(system, user))
            .send()
            .map_err(|error| http::map_error(error, self.timeout))?;
        let value: Value = http::check(response)?
            .json()
            .map_err(|error| http::map_error(error, self.timeout))?;
        value["choices"]
            .as_array()
            .into_iter()
            .flatten()
            .find_map(|choice| choice["message"]["content"].as_str().filter(|text| !text.trim().is_empty()))
            .map(str::to_owned)
            .ok_or(CoachError::EmptyReply)
    }

    fn describe(&self) -> String {
        format!("{} · {}", self.model, self.base_url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::http::test_server;

    fn backend(base: &str, key: Option<&str>, effort: &str) -> OpenAi {
        OpenAi::new(&CoachConfig {
            openai_base_url: base.to_owned(),
            openai_api_key: key.map(str::to_owned),
            openai_api_key_env: "SUBTEXT_TEST_NO_SUCH_ENV".to_owned(),
            reasoning_effort: effort.to_owned(),
            timeout_ms: 5000,
            ..CoachConfig::default()
        })
    }

    #[test]
    fn posts_chat_completions_with_bearer_key() {
        let (base, seen) = test_server::once(200, r#"{"choices":[{"message":{"content":"{\"ok\":1}"}}]}"#);
        let reply = backend(&base, Some("sk-test"), "none").complete("SYS", "USER").unwrap();
        assert_eq!(reply, r#"{"ok":1}"#);
        let seen = seen.join().unwrap();
        assert!(seen.head.starts_with("POST /chat/completions"));
        assert!(seen.head.to_lowercase().contains("authorization: bearer sk-test"));
        let body: Value = serde_json::from_str(&seen.body).unwrap();
        assert_eq!(body["messages"][0]["content"], "SYS");
        assert_eq!(body["messages"][1]["content"], "USER");
        assert_eq!(body["reasoning_effort"], "none");
    }

    #[test]
    fn empty_effort_sends_no_reasoning_parameter() {
        let body = backend("http://x", Some("k"), "").request_body("s", "u");
        assert!(body.get("reasoning_effort").is_none() && body.get("thinking").is_none());
    }

    #[test]
    fn zhipu_uses_the_thinking_switch() {
        let body = backend("https://open.bigmodel.cn/api/paas/v4", Some("k"), "none").request_body("s", "u");
        assert_eq!(body["thinking"]["type"], "disabled");
        assert!(body.get("reasoning_effort").is_none());
    }

    #[test]
    fn missing_key_fails_before_any_request() {
        assert!(matches!(
            backend("http://127.0.0.1:1", None, "none").complete("s", "u"),
            Err(CoachError::MissingApiKey(_))
        ));
    }

    #[test]
    fn http_errors_carry_status_and_body() {
        let (base, _seen) = test_server::once(401, r#"{"error":"bad key"}"#);
        let error = backend(&base, Some("k"), "none").complete("s", "u").unwrap_err();
        assert!(matches!(error, CoachError::Api { status: 401, ref body } if body.contains("bad key")));
    }

    #[test]
    fn empty_content_is_an_error() {
        let (base, _seen) = test_server::once(200, r#"{"choices":[{"message":{"content":"  "}}]}"#);
        assert!(matches!(
            backend(&base, Some("k"), "none").complete("s", "u"),
            Err(CoachError::EmptyReply)
        ));
    }
}
