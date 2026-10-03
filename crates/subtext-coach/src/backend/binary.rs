//! 找本机命令行工具：输入法进程由系统拉起，PATH 里没有用户装的东西，得自己找。

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// 登录 shell 里查一次命令的最长时间。
const SHELL_LOOKUP_TIMEOUT: Duration = Duration::from_secs(3);

/// 常见安装位置，`~` 在运行时展开。
const CANDIDATE_DIRS: &[&str] = &[
    "~/.local/bin",
    "~/.claude/local",
    "/opt/homebrew/bin",
    "/usr/local/bin",
    "~/.npm-global/bin",
    "~/.bun/bin",
    "~/.volta/bin",
    "~/.cargo/bin",
];

/// 找 `name` 的可执行文件：先看配置给的路径，再看常见目录，最后问登录 shell。
pub fn find(name: &str, configured: &str) -> Option<PathBuf> {
    let configured = configured.trim();
    if !configured.is_empty() {
        let path = expand_home(configured);
        return is_executable(&path).then_some(path);
    }
    CANDIDATE_DIRS
        .iter()
        .map(|dir| expand_home(dir).join(name))
        .find(|path| is_executable(path))
        .or_else(|| ask_login_shell(name))
}

/// 给子进程用的 PATH：常见目录在前，保证 `claude` 这类脚本的 `env node` 也找得到。
pub fn search_path() -> String {
    let mut parts: Vec<String> = CANDIDATE_DIRS
        .iter()
        .map(|dir| expand_home(dir).to_string_lossy().into_owned())
        .collect();
    parts.push("/usr/bin".to_owned());
    parts.push("/bin".to_owned());
    parts.join(":")
}

fn expand_home(path: &str) -> PathBuf {
    match (path.strip_prefix("~/"), std::env::var_os("HOME")) {
        (Some(rest), Some(home)) => Path::new(&home).join(rest),
        _ => PathBuf::from(path),
    }
}

fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

/// `zsh -lc 'command -v name'`：读到用户的 `.zprofile` 里设的 PATH。超时就放弃。
fn ask_login_shell(name: &str) -> Option<PathBuf> {
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return None;
    }
    let mut child = Command::new("/bin/zsh")
        .args(["-lc", &format!("command -v {name}")])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed() < SHELL_LOOKUP_TIMEOUT => {
                std::thread::sleep(Duration::from_millis(20));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    let mut output = String::new();
    std::io::Read::read_to_string(&mut child.stdout.take()?, &mut output).ok()?;
    let path = PathBuf::from(output.lines().last()?.trim());
    is_executable(&path).then_some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configured_path_must_exist_and_be_executable() {
        assert!(find("sh", "/bin/sh").is_some());
        assert!(find("sh", "/definitely/not/here").is_none());
        assert!(find("sh", "/etc/hosts").is_none());
    }

    #[test]
    fn search_path_lists_homebrew_before_system() {
        let path = search_path();
        assert!(path.find("/opt/homebrew/bin") < path.find("/usr/bin"));
    }

    #[test]
    fn shell_lookup_finds_standard_tools_and_rejects_odd_names() {
        assert!(ask_login_shell("ls").is_some());
        assert!(ask_login_shell("ls; rm -rf /").is_none());
    }
}
