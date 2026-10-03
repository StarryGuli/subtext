//! 跑一个子进程：从 stdin 喂提示、限时、超时杀掉整个进程组，不留后台进程。

use std::io::{Read, Write};
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::CoachError;

/// 子进程的输出。
#[derive(Debug)]
pub struct Output {
    pub stdout: String,

    pub stderr: String,

    /// 退出码为 0。
    pub success: bool,

    /// 退出状态的文字，如 `exit status: 1`。
    pub status: String,
}

impl Output {
    /// 非零退出转成 [`CoachError::CommandFailed`]，原因取 stderr，没有就取 stdout 的开头。
    pub fn into_success(self, label: &str) -> Result<Self, CoachError> {
        if self.success {
            return Ok(self);
        }
        let detail = if self.stderr.trim().is_empty() { &self.stdout } else { &self.stderr };
        Err(CoachError::CommandFailed {
            command: label.to_owned(),
            status: self.status.clone(),
            stderr: detail.trim().chars().take(STDERR_CHARS).collect(),
        })
    }
}

/// stderr 进错误信息时最多留这么多字符。
const STDERR_CHARS: usize = 400;

/// 运行 `command`，把 `stdin` 写进去；`label` 只用于错误信息。非零退出不算错误，由调用方看 [`Output::success`]。
pub fn run(
    mut command: Command,
    stdin: &str,
    timeout: Duration,
    label: &str,
) -> Result<Output, CoachError> {
    let spawn_error = |source| CoachError::Spawn {
        command: label.to_owned(),
        source,
    };
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()
        .map_err(spawn_error)?;
    let pid = child.id();

    let mut input = child.stdin.take().ok_or_else(|| CoachError::EmptyReply)?;
    let payload = stdin.to_owned();
    let writer = std::thread::spawn(move || {
        // 子进程提前退出时写会失败，不是错误
        let _ = input.write_all(payload.as_bytes());
    });
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());

    let started = Instant::now();
    let status = loop {
        match child.try_wait().map_err(spawn_error)? {
            Some(status) => break Some(status),
            None if started.elapsed() >= timeout => break None,
            None => std::thread::sleep(Duration::from_millis(15)),
        }
    };
    let Some(status) = status else {
        kill_group(pid);
        let _ = child.kill();
        let _ = child.wait();
        let _ = writer.join();
        return Err(CoachError::Timeout(timeout.as_millis() as u64));
    };
    let _ = writer.join();
    Ok(Output {
        stdout: stdout.join().unwrap_or_default(),
        stderr: stderr.join().unwrap_or_default(),
        success: status.success(),
        status: status.to_string(),
    })
}

/// 另起线程读完一根管道，免得子进程写满缓冲区卡住。
fn drain<R: Read + Send + 'static>(pipe: Option<R>) -> std::thread::JoinHandle<String> {
    std::thread::spawn(move || {
        let mut text = String::new();
        if let Some(mut pipe) = pipe {
            let mut bytes = Vec::new();
            let _ = pipe.read_to_end(&mut bytes);
            text = String::from_utf8_lossy(&bytes).into_owned();
        }
        text
    })
}

/// 杀掉整个进程组：`claude` 这类工具会再起子进程。
fn kill_group(pid: u32) {
    let _ = Command::new("/bin/kill")
        .args(["-KILL", &format!("-{pid}")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sh(script: &str) -> Command {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", script]);
        command
    }

    #[test]
    fn feeds_stdin_and_captures_stdout() {
        let output = run(sh("cat"), "hello", Duration::from_secs(5), "cat").unwrap();
        assert_eq!(output.stdout, "hello");
    }

    #[test]
    fn nonzero_exit_reports_stderr() {
        let error = run(sh("echo boom >&2; exit 3"), "", Duration::from_secs(5), "x")
            .unwrap()
            .into_success("x")
            .unwrap_err();
        assert!(matches!(&error, CoachError::CommandFailed { stderr, .. } if stderr == "boom"));
    }

    #[test]
    fn failure_detail_falls_back_to_stdout() {
        let error = run(sh("echo from-stdout; exit 1"), "", Duration::from_secs(5), "x")
            .unwrap()
            .into_success("x")
            .unwrap_err();
        assert!(matches!(&error, CoachError::CommandFailed { stderr, .. } if stderr == "from-stdout"));
    }

    #[test]
    fn timeout_kills_the_process_group() {
        let started = Instant::now();
        let error = run(sh("sleep 30 & sleep 30"), "", Duration::from_millis(200), "sleep").unwrap_err();
        assert!(matches!(error, CoachError::Timeout(200)));
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn large_output_does_not_deadlock() {
        let output = run(sh("head -c 500000 /dev/zero | tr '\\0' a"), "", Duration::from_secs(10), "big").unwrap();
        assert_eq!(output.stdout.len(), 500_000);
    }

    #[test]
    fn missing_binary_is_a_spawn_error() {
        let error = run(Command::new("/no/such/binary"), "", Duration::from_secs(1), "nope").unwrap_err();
        assert!(matches!(error, CoachError::Spawn { .. }));
    }
}
