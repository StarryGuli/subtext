//! 配置：`~/Library/Application Support/Subtext/config.toml` 里的 `[coach]` 一节，缺省值照 [`CoachConfig::default`]。

use std::path::PathBuf;

use serde::Deserialize;
use subtext_coach::CoachConfig;

#[derive(Default, Deserialize)]
#[serde(default)]
struct File {
    coach: CoachConfig,
}

/// 数据目录；拿不到 HOME 时返回 `None`。
pub fn data_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join("Library/Application Support/Subtext"))
}

/// 读配置；文件不存在用缺省，写坏了也用缺省并记一条日志，窗口总得能打开。
pub fn load() -> CoachConfig {
    // 从访达或登录项启动的应用拿不到 shell 里的环境变量：密钥放在数据目录的 `.env` 里
    if let Some(dir) = data_dir() {
        let _ = dotenvy::from_path(dir.join(".env"));
    }
    let mut config = data_dir()
        .map(|dir| dir.join("config.toml"))
        .and_then(|path| std::fs::read_to_string(path).ok())
        .map(|text| match toml::from_str::<File>(&text) {
            Ok(file) => file.coach,
            Err(error) => {
                tracing::warn!(%error, "config.toml 读不懂，用缺省配置");
                CoachConfig::default()
            }
        })
        .unwrap_or_default();
    config.enabled = true;
    config
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_file_fills_defaults() {
        let file: File = toml::from_str("[coach]\nbackend = \"openai\"\n").unwrap();
        assert_eq!(file.coach.backend, subtext_coach::BackendKind::OpenAi);
        assert_eq!(file.coach.claude_model, "haiku");
    }
}
