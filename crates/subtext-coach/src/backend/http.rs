//! 两个 API 后端共用的 HTTP 细节：客户端、错误响应的整理。

use std::time::Duration;

use reqwest::blocking::{Client, Response};

use crate::CoachError;

/// 错误响应体进错误信息时最多留这么多字符。
const BODY_CHARS: usize = 400;

pub fn client(timeout: Duration) -> Result<Client, CoachError> {
    Ok(Client::builder()
        .timeout(timeout)
        .user_agent(concat!("subtext/", env!("CARGO_PKG_VERSION")))
        .build()?)
}

/// 非 2xx 转成带状态码与响应体开头的错误；超时单独归类。
pub fn check(response: Response) -> Result<Response, CoachError> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let body = response.text().unwrap_or_default();
    Err(CoachError::Api {
        status: status.as_u16(),
        body: body.chars().take(BODY_CHARS).collect(),
    })
}

/// 读 SSE 流（`text/event-stream`）：每个 `data:` 行的内容交给 `on_data`，直到流结束或收到 `[DONE]`。
///
/// 有的兼容接口不认 `stream` 参数、直接回一整段普通 JSON：没见到任何 `data:` 行时，把读到的整段正文原样返回，
/// 让调用方按非流式的格式再解析一遍。
pub fn read_sse(
    response: Response,
    on_data: &mut dyn FnMut(&str),
) -> Result<Option<String>, CoachError> {
    use std::io::{BufRead, BufReader};

    let mut saw_data = false;
    let mut plain = String::new();
    for line in BufReader::new(response).lines() {
        let line = line?;
        match line.strip_prefix("data:") {
            Some(data) => {
                saw_data = true;
                let data = data.trim();
                if data == "[DONE]" {
                    break;
                }
                if !data.is_empty() {
                    on_data(data);
                }
            }
            None if !saw_data => {
                plain.push_str(&line);
                plain.push('\n');
            }
            None => {}
        }
    }
    Ok((!saw_data && !plain.trim().is_empty()).then_some(plain))
}

/// reqwest 的超时错误换成统一的 [`CoachError::Timeout`]。
pub fn map_error(error: reqwest::Error, timeout: Duration) -> CoachError {
    if error.is_timeout() {
        CoachError::Timeout(timeout.as_millis() as u64)
    } else {
        CoachError::Http(error)
    }
}

#[cfg(test)]
pub mod test_server {
    //! 起一个只答一次的本地 HTTP 服务，返回它收到的请求，给后端测试用。

    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread::JoinHandle;

    /// 服务收到的请求。
    pub struct Seen {
        pub head: String,

        pub body: String,
    }

    /// 返回 (基础地址, 等待请求的句柄)。
    pub fn once(status: u16, response_body: &'static str) -> (String, JoinHandle<Seen>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = Vec::new();
            let mut chunk = [0u8; 4096];
            let (head, body) = loop {
                let read = stream.read(&mut chunk).unwrap();
                buffer.extend_from_slice(&chunk[..read]);
                if let Some(end) = find(&buffer, b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&buffer[..end]).into_owned();
                    let length = head
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .and_then(|value| value.trim().parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    while buffer.len() < end + 4 + length {
                        let read = stream.read(&mut chunk).unwrap();
                        buffer.extend_from_slice(&chunk[..read]);
                    }
                    let body =
                        String::from_utf8_lossy(&buffer[end + 4..end + 4 + length]).into_owned();
                    break (head, body);
                }
            };
            let reply = format!(
                "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{response_body}",
                response_body.len()
            );
            stream.write_all(reply.as_bytes()).unwrap();
            Seen { head, body }
        });
        (base, handle)
    }

    fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack
            .windows(needle.len())
            .position(|window| window == needle)
    }
}
