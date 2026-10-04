//! 结果缓存：同一段文字再来一次，不必再问后端（也不再花额度）。
//!
//! 复制解码、屏幕阅读预解码、重启输入法之后都共用同一份：缓存可以落盘（追加一行 JSON），启动时读回来。
//! 键只看文字，不看当前是哪个应用：同一条消息换个应用复制、或者先被屏幕阅读预解码过，都算命中。

use std::collections::{HashMap, VecDeque};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use serde::{Deserialize, Serialize};

use crate::{CoachOutput, CoachRequest, Mode};

/// 缓存条数；超过丢最老的。
const CAPACITY: usize = 300;

/// 磁盘文件里的行数超过容量的这么多倍就整理一次（把被淘汰的条目丢掉）。
const COMPACT_FACTOR: usize = 2;

/// 磁盘格式版本，改了结构就加一，读到别的版本的行直接跳过。
const FORMAT_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct Line {
    v: u32,

    k: u64,

    output: CoachOutput,
}

#[derive(Default)]
struct Inner {
    entries: HashMap<u64, CoachOutput>,

    /// 插入顺序，满了丢最老的。
    order: VecDeque<u64>,

    /// 落盘的文件；`None` 只放内存。
    path: Option<PathBuf>,

    /// 文件里现在有多少行，用来决定什么时候整理。
    lines_on_disk: usize,
}

/// 可以在多个后台线程之间共享的缓存（克隆只是多一个句柄）。
#[derive(Clone, Default)]
pub struct SharedCache {
    inner: Arc<Mutex<Inner>>,
}

impl SharedCache {
    /// 只放内存，重启就没。
    pub fn in_memory() -> Self {
        Self::default()
    }

    /// 落盘到 `path`：先读回已有的，之后每条新结果追加一行。文件读不了、行坏了都只是跳过，不影响使用。
    pub fn persistent(path: PathBuf) -> Self {
        let mut inner = Inner::default();
        if let Ok(text) = std::fs::read_to_string(&path) {
            for line in text.lines() {
                inner.lines_on_disk += 1;
                if let Ok(parsed) = serde_json::from_str::<Line>(line)
                    && parsed.v == FORMAT_VERSION
                {
                    inner.put(parsed.k, parsed.output);
                }
            }
        }
        inner.path = Some(path);
        Self {
            inner: Arc::new(Mutex::new(inner)),
        }
    }

    /// 缓存键：模式加文字，写回复 / 改稿再加上影响语气的上下文（当前应用、对方的消息）。
    /// 文字只留字母数字并转小写：OCR 与剪贴板的标点、空白、大小写差异不该让命中落空。
    pub fn key(request: &CoachRequest) -> u64 {
        let mut hash = Fnv::new();
        hash.write(&[mode_tag(request.mode)]);
        hash.write(normalized(&request.text).as_bytes());
        if matches!(request.mode, Mode::Compose | Mode::Edit) {
            hash.write(&[0xff]);
            hash.write(
                request
                    .context
                    .app
                    .as_deref()
                    .unwrap_or_default()
                    .as_bytes(),
            );
            hash.write(&[0xff]);
            hash.write(
                request
                    .context
                    .peer_message
                    .as_deref()
                    .map(normalized)
                    .unwrap_or_default()
                    .as_bytes(),
            );
        }
        hash.finish()
    }

    pub fn get(&self, key: u64) -> Option<CoachOutput> {
        self.lock().entries.get(&key).cloned()
    }

    pub fn insert(&self, key: u64, output: CoachOutput) {
        let mut inner = self.lock();
        inner.append_to_disk(key, &output);
        inner.put(key, output);
        inner.compact_if_needed();
    }

    /// 清空内存与磁盘上的历史。
    pub fn clear(&self) {
        let mut inner = self.lock();
        inner.entries.clear();
        inner.order.clear();
        inner.lines_on_disk = 0;
        if let Some(path) = &inner.path {
            let _ = std::fs::remove_file(path);
        }
    }

    pub fn len(&self) -> usize {
        self.lock().entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Inner {
    fn put(&mut self, key: u64, output: CoachOutput) {
        if self.entries.insert(key, output).is_none() {
            self.order.push_back(key);
        } else {
            // 重新放进来的当作最新
            self.order.retain(|existing| *existing != key);
            self.order.push_back(key);
        }
        while self.order.len() > CAPACITY {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
    }

    fn append_to_disk(&mut self, key: u64, output: &CoachOutput) {
        let Some(path) = &self.path else {
            return;
        };
        let line = Line {
            v: FORMAT_VERSION,
            k: key,
            output: output.clone(),
        };
        let Ok(mut json) = serde_json::to_string(&line) else {
            return;
        };
        json.push('\n');
        if write_private(path, json.as_bytes(), true).is_ok() {
            self.lines_on_disk += 1;
        }
    }

    /// 文件里堆了太多被淘汰的旧行：按当前内容重写一遍。
    fn compact_if_needed(&mut self) {
        let Some(path) = self.path.clone() else {
            return;
        };
        if self.lines_on_disk <= CAPACITY * COMPACT_FACTOR {
            return;
        }
        let mut text = String::new();
        for key in &self.order {
            if let Some(output) = self.entries.get(key)
                && let Ok(json) = serde_json::to_string(&Line {
                    v: FORMAT_VERSION,
                    k: *key,
                    output: output.clone(),
                })
            {
                text.push_str(&json);
                text.push('\n');
            }
        }
        if write_private(&path, text.as_bytes(), false).is_ok() {
            self.lines_on_disk = self.order.len();
        }
    }
}

/// 写文件，权限 0600（里面是用户复制 / 输入过的文字的解读，只给自己看）。`append` 为假就覆盖。
fn write_private(path: &Path, bytes: &[u8], append: bool) -> std::io::Result<()> {
    use std::os::unix::fs::OpenOptionsExt;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut options = std::fs::OpenOptions::new();
    options.create(true).mode(0o600);
    if append {
        options.append(true);
    } else {
        options.write(true).truncate(true);
    }
    options.open(path)?.write_all(bytes)
}

fn mode_tag(mode: Mode) -> u8 {
    match mode {
        Mode::Decode => 1,
        Mode::Compose => 2,
        Mode::Edit => 3,
        Mode::Screen => 4,
    }
}

fn normalized(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// FNV-1a：键要在不同次启动、不同 Rust 版本之间保持一致才能落盘，标准库的哈希不保证这一点。
struct Fnv(u64);

impl Fnv {
    fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CoachContext, Decoded};

    fn request(mode: Mode, text: &str, app: Option<&str>) -> CoachRequest {
        CoachRequest {
            id: 1,
            mode,
            text: text.to_owned(),
            context: CoachContext {
                app: app.map(str::to_owned),
                ..CoachContext::default()
            },
        }
    }

    fn decoded(situation: &str) -> CoachOutput {
        CoachOutput::Decode(Decoded {
            situation: situation.to_owned(),
            ..Decoded::default()
        })
    }

    fn temp_file(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("subtext-cache-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("history.jsonl")
    }

    #[test]
    fn decode_key_ignores_the_app_and_formatting_but_not_the_words() {
        let a = SharedCache::key(&request(
            Mode::Decode,
            "Hey, no rush at all!",
            Some("com.tencent.xinWeChat"),
        ));
        let b = SharedCache::key(&request(
            Mode::Decode,
            "hey  no rush at all",
            Some("com.apple.Safari"),
        ));
        assert_eq!(a, b);
        assert_ne!(
            a,
            SharedCache::key(&request(Mode::Decode, "hey no rush at some", None))
        );
        // 同一段文字，不同模式不能互相命中
        assert_ne!(
            a,
            SharedCache::key(&request(Mode::Screen, "hey no rush at all", None))
        );
    }

    #[test]
    fn compose_key_depends_on_the_app_because_tone_does() {
        let mail = SharedCache::key(&request(
            Mode::Compose,
            "我这周做不完",
            Some("com.apple.mail"),
        ));
        let chat = SharedCache::key(&request(
            Mode::Compose,
            "我这周做不完",
            Some("com.hnc.Discord"),
        ));
        assert_ne!(mail, chat);
    }

    #[test]
    fn keys_are_stable_across_runs() {
        // 写死一个值：键要能落盘后再读回，算法变了这里必须有人注意到
        assert_eq!(
            SharedCache::key(&request(Mode::Decode, "no rush at all", None)),
            SharedCache::key(&request(Mode::Decode, "No rush at all.", None))
        );
        assert_eq!(Fnv::new().finish(), 0xcbf2_9ce4_8422_2325);
    }

    #[test]
    fn persists_and_restores_across_instances() {
        let path = temp_file("persist");
        let key = SharedCache::key(&request(Mode::Decode, "see you tomorrow at the lab", None));
        {
            let cache = SharedCache::persistent(path.clone());
            assert!(cache.is_empty());
            cache.insert(key, decoded("约明天实验室见"));
        }
        let restored = SharedCache::persistent(path.clone());
        assert_eq!(restored.get(key), Some(decoded("约明天实验室见")));
        // 文件只给自己看
        let mode = std::fs::metadata(&path).unwrap().permissions();
        assert_eq!(
            std::os::unix::fs::PermissionsExt::mode(&mode) & 0o777,
            0o600
        );
        restored.clear();
        assert!(SharedCache::persistent(path).is_empty());
    }

    #[test]
    fn corrupt_lines_and_other_versions_are_skipped() {
        let path = temp_file("corrupt");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let good = serde_json::to_string(&Line {
            v: FORMAT_VERSION,
            k: 7,
            output: decoded("好"),
        })
        .unwrap();
        let old = serde_json::to_string(&Line {
            v: 99,
            k: 8,
            output: decoded("旧"),
        })
        .unwrap();
        std::fs::write(&path, format!("not json\n{good}\n{old}\n{{\"v\":1}}\n")).unwrap();
        let cache = SharedCache::persistent(path);
        assert_eq!(cache.len(), 1);
        assert!(cache.get(7).is_some() && cache.get(8).is_none());
    }

    #[test]
    fn evicts_the_oldest_and_compacts_the_file() {
        let path = temp_file("compact");
        let cache = SharedCache::persistent(path.clone());
        for key in 0..(CAPACITY as u64 * 2 + 20) {
            cache.insert(key, decoded("x"));
        }
        assert_eq!(cache.len(), CAPACITY);
        assert!(cache.get(0).is_none() && cache.get(CAPACITY as u64 * 2 + 19).is_some());
        let lines = std::fs::read_to_string(&path).unwrap().lines().count();
        assert!(
            lines <= CAPACITY * COMPACT_FACTOR + 1,
            "文件没有整理：{lines} 行"
        );
        // 整理之后重新读回来仍是最新的那些
        let reloaded = SharedCache::persistent(path);
        assert!(reloaded.get(CAPACITY as u64 * 2 + 19).is_some());
    }

    #[test]
    fn handles_share_one_store() {
        let a = SharedCache::in_memory();
        let b = a.clone();
        a.insert(1, decoded("x"));
        assert!(b.get(1).is_some());
    }
}
