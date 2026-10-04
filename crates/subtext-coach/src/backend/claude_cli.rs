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
            // stream-json 要带 --verbose；--include-partial-messages 让文本一段一段地来，面板才能边生成边显示
            .args(["-p", "--output-format", "stream-json", "--verbose"])
            .arg("--include-partial-messages")
            .args(["--system-prompt", system])
            // 教练只回话：不要工具、斜杠命令，也不留会话；
            // 只读项目级配置（临时目录里没有），免得用户的插件钩子、全局记忆与权限规则混进来
            .args(["--tools", ""])
            .args(["--disable-slash-commands", "--no-session-persistence"])
            .args(["--setting-sources", "project"])
            // 教练要快不要深思：命令行本身启动就要几秒，推理强度压到最低
            .args(["--effort", "low"])
            .env("PATH", binary::search_path())
            // 关掉扩展思考：几个字的小请求它也要先「想」几秒，教练要的是快
            .env("MAX_THINKING_TOKENS", "0")
            .current_dir(std::env::temp_dir());
        if !self.model.trim().is_empty() {
            command.args(["--model", self.model.trim()]);
        }
        command
    }
}

impl Backend for ClaudeCli {
    fn complete(&self, system: &str, user: &str) -> Result<String, CoachError> {
        self.stream(system, user, &mut |_| {})
    }

    fn stream(
        &self,
        system: &str,
        user: &str,
        on_text: &mut dyn FnMut(&str),
    ) -> Result<String, CoachError> {
        let binary = self.binary()?;
        let mut streamed = String::new();
        let output = process::run_lines(
            self.command(&binary, system),
            user,
            self.timeout,
            "claude",
            |line| {
                if let Some(text) = text_delta(line) {
                    streamed.push_str(&text);
                    on_text(&text);
                }
            },
        )?;
        // 登录过期这类错误，claude 以非零退出并把原因写在 stdout 里 type=result 的那一行
        match parse_result(&output.stdout) {
            Err(CoachError::BadReply(_)) if !output.success => {
                output.into_success("claude").map(|_| String::new())
            }
            Err(CoachError::BadReply(_)) if !streamed.trim().is_empty() => Ok(streamed),
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

/// stream-json 的一行里要是一段新文本（`stream_event` → `content_block_delta` → `text_delta`）就取出来。
fn text_delta(line: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    if value["type"] != "stream_event" || value["event"]["type"] != "content_block_delta" {
        return None;
    }
    let delta = &value["event"]["delta"];
    (delta["type"] == "text_delta")
        .then(|| delta["text"].as_str().map(str::to_owned))
        .flatten()
}

/// 从输出里找 `type=result` 的那一行（流式输出有很多行，最后一条是结果）。
fn parse_result(stdout: &str) -> Result<String, CoachError> {
    let line = stdout
        .lines()
        .rev()
        .find(|line| line.contains("\"type\":\"result\"") || line.contains("\"type\": \"result\""))
        .unwrap_or_else(|| stdout.trim());
    let parsed: CliResult = serde_json::from_str(line.trim())
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
    fn streams_text_deltas_and_returns_the_result_line() {
        let dir = tempdir("stream");
        let script = r#"cat > /dev/null
printf '%s\n' '{"type":"system","subtype":"init"}'
printf '%s\n' '{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"thinking_delta","thinking":"hmm"}}}'
printf '%s\n' '{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"{\"a\":"}}}'
printf '%s\n' '{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"1}"}}}'
printf '%s\n' '{"type":"result","subtype":"success","is_error":false,"result":"{\"a\":1}"}'"#;
        let backend = backend(fake_claude(&dir, script));
        let mut deltas = Vec::new();
        let full = backend
            .stream("s", "u", &mut |delta| deltas.push(delta.to_owned()))
            .unwrap();
        assert_eq!(full, r#"{"a":1}"#);
        // 思考块的增量不算正文
        assert_eq!(deltas, [r#"{"a":"#, "1}"]);
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
