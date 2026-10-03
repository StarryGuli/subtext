//! GitHub Releases 接口返回的一个版本，转成索引里的 [`Release`]。

use serde::Deserialize;

use super::{Asset, Release};

/// `GET /repos/<owner>/<repo>/releases` 里取检查更新用得着的字段。
#[derive(Debug, Deserialize)]
pub(super) struct GithubRelease {
    tag_name: String,

    #[serde(default)]
    draft: bool,

    #[serde(default)]
    prerelease: bool,

    #[serde(default)]
    published_at: Option<String>,

    #[serde(default)]
    body: Option<String>,

    #[serde(default)]
    assets: Vec<GithubAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubAsset {
    name: String,
}

/// 更新日志最多取这么多行。
const MAX_NOTES: usize = 8;

impl GithubRelease {
    /// 草稿不算；没有 macOS 安装包的版本照样转，由 [`super::Index::newest`] 按平台过滤。
    pub(super) fn into_release(self) -> Option<Release> {
        if self.draft {
            return None;
        }
        let version = self.tag_name.trim_start_matches('v').to_owned();
        let channel = channel_of(&version, self.prerelease);
        let assets = self.assets.iter().flat_map(|asset| installers(&asset.name)).collect();
        let notes = self
            .body
            .unwrap_or_default()
            .lines()
            .map(|line| line.trim().trim_start_matches(['-', '*']).trim())
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .take(MAX_NOTES)
            .map(str::to_owned)
            .collect();
        Some(Release {
            version,
            date: self
                .published_at
                .unwrap_or_default()
                .chars()
                .take(10)
                .collect(),
            channel,
            notes,
            assets,
        })
    }
}

/// 版本号里带 alpha / beta / rc 就按它分渠道；GitHub 标了预发布但版本号没写的归 beta。
fn channel_of(version: &str, prerelease: bool) -> String {
    let lowered = version.to_ascii_lowercase();
    for name in ["alpha", "beta", "rc"] {
        if lowered.contains(name) {
            return name.to_owned();
        }
    }
    if prerelease { "beta" } else { "stable" }.to_owned()
}

/// 安装包文件名里认 `.pkg` 与 CPU（`arm64` / `x86_64`，`universal` 两种都算）。
fn installers(name: &str) -> Vec<Asset> {
    let lowered = name.to_ascii_lowercase();
    if !lowered.ends_with(".pkg") {
        return Vec::new();
    }
    let cpus: &[&str] = if lowered.contains("universal") {
        &["arm64", "x86_64"]
    } else if lowered.contains("arm64") {
        &["arm64"]
    } else if lowered.contains("x86_64") {
        &["x86_64"]
    } else {
        return Vec::new();
    };
    cpus.iter()
        .map(|cpu| Asset {
            platform: "macos".to_owned(),
            cpu: (*cpu).to_owned(),
        })
        .collect()
}
