//! `[coach]` 配置分节。默认**关闭**；开启后复制的英文与上屏的中文会发往所选后端。

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// 用哪个后端回答。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BackendKind {
    /// 本机 Claude Code：`claude -p`，走订阅额度。
    #[serde(rename = "claude-cli")]
    ClaudeCli,

    /// 本机 Codex：`codex exec`。
    #[serde(rename = "codex-cli")]
    CodexCli,

    /// 自填的 OpenAI 兼容接口。
    #[serde(rename = "openai")]
    OpenAi,

    /// Anthropic Messages API。
    #[serde(rename = "anthropic")]
    Anthropic,
}

impl BackendKind {
    pub const ALL: [Self; 4] = [Self::ClaudeCli, Self::CodexCli, Self::OpenAi, Self::Anthropic];

    /// 配置文件里的写法。
    pub fn key(self) -> &'static str {
        match self {
            Self::ClaudeCli => "claude-cli",
            Self::CodexCli => "codex-cli",
            Self::OpenAi => "openai",
            Self::Anthropic => "anthropic",
        }
    }

    /// 设置里显示的名字。
    pub fn label(self) -> &'static str {
        match self {
            Self::ClaudeCli => "本机 Claude Code",
            Self::CodexCli => "本机 Codex",
            Self::OpenAi => "OpenAI 兼容接口",
            Self::Anthropic => "Anthropic API",
        }
    }

    /// 是否走本机命令行（不需要密钥）。
    pub fn is_local_cli(self) -> bool {
        matches!(self, Self::ClaudeCli | Self::CodexCli)
    }
}

/// 学习者画像的缺省值：不假设用户是谁，只写通用的事实。
const DEFAULT_PROFILE: &str = "母语中文，英语读写流利但不是母语；语法与词汇基本没问题，缺口主要在俚语、语域与潜台词。常见场景：与教授和实验室导师、留学申请、美国同学室友、技术社区（Discord、Reddit、GitHub）的往来。";

/// 缺省不处理这些应用里的内容：密码管理器与钥匙串。
const DEFAULT_SKIP_APPS: &[&str] = &[
    "com.1password.1password",
    "com.agilebits.onepassword7",
    "com.bitwarden.desktop",
    "com.apple.keychainaccess",
    "com.apple.passwords",
    "com.lastpass.lastpass",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CoachConfig {
    /// 总开关。
    pub enabled: bool,

    pub backend: BackendKind,

    /// 复制了英文就自动解码。
    pub auto_decode: bool,

    /// 上屏了一句中文就自动给出英文表达。
    pub auto_compose: bool,

    /// `claude` 可执行文件的路径；留空则自动查找。
    pub claude_path: String,

    /// 传给 `claude --model` 的模型，缺省用快的 haiku。
    pub claude_model: String,

    /// `codex` 可执行文件的路径；留空则自动查找。
    pub codex_path: String,

    /// 传给 `codex -m` 的模型；留空用 Codex 自己的缺省。
    pub codex_model: String,

    /// OpenAI 兼容接口地址（不含 `/chat/completions`）。
    pub openai_base_url: String,

    pub openai_model: String,

    /// 密钥。留空则读 `openai_api_key_env` 指定的环境变量。
    pub openai_api_key: Option<String>,

    pub openai_api_key_env: String,

    /// 随请求发 `reasoning_effort`：`none` 关掉思考（教练要快）；留空不发，给不认这个参数的接口。
    pub reasoning_effort: String,

    pub anthropic_model: String,

    /// 密钥。留空则读 `anthropic_api_key_env` 指定的环境变量。
    pub anthropic_api_key: Option<String>,

    pub anthropic_api_key_env: String,

    /// 单次请求超时（毫秒）。命令行后端要启动进程，给得比 API 宽。
    pub timeout_ms: u64,

    /// 单次最多发多少字符，超了不发。
    pub max_chars: usize,

    /// 学习者画像，写进系统提示。
    pub profile: String,

    /// 不处理这些应用里复制或输入的内容（bundle id）。
    pub skip_apps: Vec<String>,
}

impl Default for CoachConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            backend: BackendKind::ClaudeCli,
            auto_decode: true,
            auto_compose: true,
            claude_path: String::new(),
            claude_model: "haiku".to_owned(),
            codex_path: String::new(),
            codex_model: String::new(),
            openai_base_url: "https://api.deepseek.com".to_owned(),
            openai_model: "deepseek-v4-flash".to_owned(),
            openai_api_key: None,
            openai_api_key_env: "SUBTEXT_COACH_API_KEY".to_owned(),
            reasoning_effort: "none".to_owned(),
            anthropic_model: "claude-haiku-4-5-20251001".to_owned(),
            anthropic_api_key: None,
            anthropic_api_key_env: "ANTHROPIC_API_KEY".to_owned(),
            timeout_ms: 90_000,
            max_chars: 1500,
            profile: DEFAULT_PROFILE.to_owned(),
            skip_apps: DEFAULT_SKIP_APPS.iter().map(|app| (*app).to_owned()).collect(),
        }
    }
}

impl CoachConfig {
    pub fn timeout(&self) -> Duration {
        Duration::from_millis(self.timeout_ms)
    }

    /// OpenAI 兼容接口的密钥：配置里的优先，其次环境变量。
    pub fn resolve_openai_key(&self) -> Option<String> {
        resolve_key(self.openai_api_key.as_deref(), &self.openai_api_key_env)
    }

    /// Anthropic 的密钥：配置里的优先，其次环境变量。
    pub fn resolve_anthropic_key(&self) -> Option<String> {
        resolve_key(self.anthropic_api_key.as_deref(), &self.anthropic_api_key_env)
    }
}

fn resolve_key(configured: Option<&str>, env_name: &str) -> Option<String> {
    configured
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(str::to_owned)
        .or_else(|| std::env::var(env_name).ok())
        .map(|key| key.trim().to_owned())
        .filter(|key| !key.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_off_and_private() {
        let config = CoachConfig::default();
        assert!(!config.enabled);
        assert!(config.skip_apps.iter().any(|app| app.contains("1password")));
        assert_eq!(config.backend, BackendKind::ClaudeCli);
    }

    #[test]
    fn partial_toml_fills_the_rest() {
        let json = serde_json::json!({ "enabled": true, "backend": "openai", "max_chars": 500 });
        let config: CoachConfig = serde_json::from_value(json).unwrap();
        assert!(config.enabled && config.backend == BackendKind::OpenAi);
        assert_eq!(config.max_chars, 500);
        assert_eq!(config.claude_model, "haiku");
    }

    #[test]
    fn configured_key_beats_environment_and_blank_is_none() {
        let mut config = CoachConfig {
            openai_api_key: Some("  sk-config  ".to_owned()),
            ..CoachConfig::default()
        };
        assert_eq!(config.resolve_openai_key().as_deref(), Some("sk-config"));
        config.openai_api_key = Some("   ".to_owned());
        config.openai_api_key_env = "SUBTEXT_TEST_ENV_THAT_DOES_NOT_EXIST".to_owned();
        assert_eq!(config.resolve_openai_key(), None);
    }

    #[test]
    fn backend_keys_round_trip() {
        for kind in BackendKind::ALL {
            let json = serde_json::to_string(&kind).unwrap();
            assert_eq!(json, format!("\"{}\"", kind.key()));
        }
    }
}
