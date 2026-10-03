//! 教练请求失败的原因；一律只记日志与在面板里显示，不打扰输入。

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoachError {
    #[error("`{0}` was not found; install it or set its path in the coach settings")]
    BinaryNotFound(String),

    #[error("no API key: set it in the coach settings or the `{0}` environment variable")]
    MissingApiKey(String),

    #[error("failed to run `{command}`: {source}")]
    Spawn {
        command: String,
        source: std::io::Error,
    },

    #[error("`{command}` exited with {status}: {stderr}")]
    CommandFailed {
        command: String,
        status: String,
        stderr: String,
    },

    #[error("request timed out after {0} ms")]
    Timeout(u64),

    #[error("request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("API returned {status}: {body}")]
    Api { status: u16, body: String },

    #[error("backend returned no usable content")]
    EmptyReply,

    #[error("reply is not the expected JSON: {0}")]
    BadReply(String),

    #[error("failed to encode request: {0}")]
    Encode(#[from] serde_json::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("coach worker has stopped")]
    WorkerStopped,
}
