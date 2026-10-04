//! 本机 Claude Code：`claude -p`，提示走 stdin，走用户自己的订阅额度。

use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use serde::Deserialize;

use super::{Backend, binary, process};
use crate::{CoachConfig, CoachError};

pub struct ClaudeCli {
    configured_path: String,

    model: String,

    timeout: Duration,
}

/// `--output-format json` 的结果对象。
#[derive(Deserialize)]
struct CliResult {
    #[serde(default)]
    result: String,

    #[serde(default)]
    is_error: bool,
}

impl ClaudeCli {
    pub fn new(config: &CoachConfig) -> Self {
        Self {
            configured_path: config.claude_path.clone(),
            model: config.claude_model.clone(),
            timeout: config.timeout(),
        }
    }

    fn binary(&self) -> Result<PathBuf, CoachError> {
        binary::find("claude", &self.configured_path)
            .ok_or_else(|| CoachError::BinaryNotFound("claude".to_owned()))
    }

    fn command(&self, binary: &PathBuf, system: &str) -> Command {
        let mut command = Command::new(binary);
        command
            .args(["-p", "--output-format", "json"])
            .args(["--system-prompt", system])
            // 教练只回话：不要工具、斜杠命令，也不留会话；
            // 只读项目级配置（临时目录里没有），免得用户的插件钩子、全局记忆与权限规则混进来
            .args(["--tools", ""])
            .args(["--disable-slash-commands", "--no-session-persistence"])
            .args(["--setting-sources", "project"])
            // 教练要快不要深思：命令行本身启动就要几秒，推理强度压到最低
            .args(["--effort", "low"])
            .env("PATH", binary::search_path())
            .current_dir(std::env::temp_dir());
        if !self.model.trim().is_empty() {
            command.args(["--model", self.model.trim()]);
        }
        command
    }
}

impl Backend for ClaudeCli {
    fn complete(&self, system: &str, user: &str) -> Result<String, CoachError> {
        let binary = self.binary()?;
        let output = process::run(self.command(&binary, system), user, self.timeout, "claude")?;
        // 登录过期这类错误，claude 以非零退出并把原因写在 stdout 的 JSON 里
        match parse_result(&output.stdout) {
            Err(CoachError::BadReply(_)) if !output.success => {
                output.into_success("claude").map(|_| String::new())
            }
            other => other,
        }
    }

    fn describe(&self) -> String {
        match self.binary() {
            Ok(path) => format!("Claude Code · {} · {}", self.model, path.display()),
            Err(_) => "Claude Code · 未找到 claude".to_owned(),
        }
    }
}

fn parse_result(stdout: &str) -> Result<String, CoachError> {
    let parsed: CliResult = serde_json::from_str(stdout.trim())
        .map_err(|error| CoachError::BadReply(format!("claude output: {error}")))?;
    if parsed.is_error {
        return Err(CoachError::CommandFailed {
            command: "claude".to_owned(),
            status: "is_error".to_owned(),
            stderr: parsed.result.chars().take(400).collect(),
        });
    }
    if parsed.result.trim().is_empty() {
        return Err(CoachError::EmptyReply);
    }
    Ok(parsed.result)
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    /// 写一个假的 `claude`：把收到的参数与 stdin 回显进 result，便于断言我们怎么调用它。
    fn fake_claude(dir: &std::path::Path, body: &str) -> String {
        let path = dir.join("claude");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path.to_string_lossy().into_owned()
    }

    fn backend(path: String) -> ClaudeCli {
        let config = CoachConfig {
            claude_path: path,
            timeout_ms: 5000,
            ..CoachConfig::default()
        };
        ClaudeCli::new(&config)
    }

    #[test]
    fn passes_the_prompt_on_stdin_and_never_on_argv() {
        let dir = tempdir("argv");
        let script = r#"input=$(cat); printf '{"type":"result","is_error":false,"result":"args=%s | stdin=%s"}' "$*" "$input""#;
        let reply = backend(fake_claude(&dir, script))
            .complete("SYSTEM RULES", "secret clipboard text")
            .unwrap();
        assert!(reply.contains("stdin=secret clipboard text"));
        assert!(reply.contains("--tools"));
        assert!(reply.contains("--setting-sources project"));
        assert!(reply.contains("--effort low"));
        assert!(reply.contains("--system-prompt SYSTEM RULES"));
        assert!(reply.contains("--model haiku"));
        assert!(
            !reply
                .split("| stdin=")
                .next()
                .unwrap()
                .contains("secret clipboard text")
        );
    }

    #[test]
    fn reports_error_results() {
        let dir = tempdir("err");
        let path = fake_claude(
            &dir,
            r#"printf '{"is_error":true,"result":"Not logged in"}'"#,
        );
        let error = backend(path).complete("s", "u").unwrap_err();
        assert!(
            matches!(error, CoachError::CommandFailed { ref stderr, .. } if stderr == "Not logged in")
        );
    }

    #[test]
    fn missing_binary_is_reported() {
        let error = backend("/no/such/claude".to_owned())
            .complete("s", "u")
            .unwrap_err();
        assert!(matches!(error, CoachError::BinaryNotFound(_)));
    }

    fn tempdir(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("subtext-coach-test-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}
