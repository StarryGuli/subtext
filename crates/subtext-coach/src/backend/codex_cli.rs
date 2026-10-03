//! 本机 Codex：`codex exec`，提示走 stdin，最终回复读 `--output-last-message` 写的文件。
//!
//! Codex 没有单独的系统提示参数，系统提示与用户消息拼成一段。

use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use super::{Backend, binary, process};
use crate::{CoachConfig, CoachError};

pub struct CodexCli {
    configured_path: String,

    model: String,

    timeout: Duration,
}

impl CodexCli {
    pub fn new(config: &CoachConfig) -> Self {
        Self {
            configured_path: config.codex_path.clone(),
            model: config.codex_model.clone(),
            timeout: config.timeout(),
        }
    }

    fn binary(&self) -> Result<PathBuf, CoachError> {
        binary::find("codex", &self.configured_path)
            .ok_or_else(|| CoachError::BinaryNotFound("codex".to_owned()))
    }
}

impl Backend for CodexCli {
    fn complete(&self, system: &str, user: &str) -> Result<String, CoachError> {
        let binary = self.binary()?;
        let reply_file = std::env::temp_dir().join(format!(
            "subtext-codex-{}-{}.txt",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.as_nanos())
        ));
        let mut command = Command::new(&binary);
        command
            .args([
                "exec",
                "--skip-git-repo-check",
                "--sandbox",
                "read-only",
                "--color",
                "never",
            ])
            .arg("--output-last-message")
            .arg(&reply_file)
            .env("PATH", binary::search_path())
            .current_dir(std::env::temp_dir());
        if !self.model.trim().is_empty() {
            command.args(["-m", self.model.trim()]);
        }
        // `-` 让 codex 从 stdin 读提示
        command.arg("-");
        let prompt = format!("{system}\n\n{user}");
        let result = process::run(command, &prompt, self.timeout, "codex");
        let reply = std::fs::read_to_string(&reply_file);
        let _ = std::fs::remove_file(&reply_file);
        let output = result?.into_success("codex")?;
        let text = reply.unwrap_or(output.stdout);
        if text.trim().is_empty() {
            return Err(CoachError::EmptyReply);
        }
        Ok(text)
    }

    fn describe(&self) -> String {
        match self.binary() {
            Ok(path) => format!("Codex · {}", path.display()),
            Err(_) => "Codex · 未找到 codex".to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    /// 假的 `codex`：把 stdin 写进 `--output-last-message` 指的文件。
    fn fake_codex(name: &str) -> String {
        let dir =
            std::env::temp_dir().join(format!("subtext-codex-test-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("codex");
        let script = r#"#!/bin/sh
out=""
while [ $# -gt 0 ]; do
  if [ "$1" = "--output-last-message" ]; then out="$2"; shift; fi
  shift
done
cat > "$out"
"#;
        std::fs::write(&path, script).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn reads_the_last_message_file_and_sends_system_plus_user() {
        let config = CoachConfig {
            codex_path: fake_codex("ok"),
            timeout_ms: 5000,
            ..CoachConfig::default()
        };
        let reply = CodexCli::new(&config).complete("SYS", "USER").unwrap();
        assert_eq!(reply, "SYS\n\nUSER");
    }

    #[test]
    fn missing_binary_is_reported() {
        let config = CoachConfig {
            codex_path: "/no/such/codex".to_owned(),
            ..CoachConfig::default()
        };
        assert!(matches!(
            CodexCli::new(&config).complete("s", "u"),
            Err(CoachError::BinaryNotFound(_))
        ));
    }
}
