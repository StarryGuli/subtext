mod asset;
mod fetch;
mod github;
mod release;

pub use asset::Asset;
pub(crate) use fetch::fetch_index;
pub use release::Release;

use subtext_platform::UpdateChannel;

use self::github::GithubRelease;

use crate::{Available, Target, UpdateError, Version};

/// 版本列表，只取检查更新用得着的字段。
#[derive(Debug, Clone)]
pub struct Index {
    /// 从新到旧。
    pub releases: Vec<Release>,
}

impl Index {
    /// 解析 GitHub Releases 接口返回的 JSON 数组。
    pub fn parse(bytes: &[u8]) -> Result<Self, UpdateError> {
        let raw: Vec<GithubRelease> = serde_json::from_slice(bytes)?;
        Ok(Self {
            releases: raw
                .into_iter()
                .filter_map(GithubRelease::into_release)
                .collect(),
        })
    }

    /// 比 `current` 新、渠道里看得到、有这台机器安装包的版本里最新的那个。
    pub fn newest(
        &self,
        current: &Version,
        target: Target,
        channel: UpdateChannel,
    ) -> Option<Available> {
        self.releases
            .iter()
            .filter(|release| channel.includes(&release.channel))
            .filter(|release| {
                release
                    .assets
                    .iter()
                    .any(|asset| asset.platform == target.platform && asset.cpu == target.cpu)
            })
            .filter_map(|release| Some((Version::parse(&release.version)?, release)))
            .filter(|(version, _)| version > current)
            .max_by(|(left, _), (right, _)| left.cmp(right))
            .map(|(_, release)| Available {
                version: release.version.clone(),
                channel: release.channel.clone(),
                date: release.date.clone(),
                notes: release.notes.clone(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAC: Target = Target {
        platform: "macos",
        cpu: "arm64",
    };
    const MAC_INTEL: Target = Target {
        platform: "macos",
        cpu: "x86_64",
    };

    fn index() -> Index {
        Index::parse(
            r###"[
              {"tag_name": "v0.2.0-beta.1", "prerelease": true, "published_at": "2026-10-20T08:00:00Z",
               "assets": [{"name": "subtext-0.2.0-beta.1-macos-arm64.pkg"}]},
              {"tag_name": "v0.1.1", "published_at": "2026-10-10T08:00:00Z", "body": "## 更新\n- 修了 a\n- 加了 b",
               "assets": [{"name": "subtext-0.1.1-macos-arm64.pkg"}, {"name": "subtext-0.1.1-macos-x86_64.pkg"},
                          {"name": "SHA256SUMS"}]},
              {"tag_name": "v0.1.0", "assets": [{"name": "subtext-0.1.0-macos-universal.pkg"}]},
              {"tag_name": "v9.9.9", "draft": true, "assets": [{"name": "subtext-9.9.9-macos-arm64.pkg"}]}
            ]"###
            .as_bytes(),
        )
        .unwrap()
    }

    fn newest(current: &str, target: Target, channel: UpdateChannel) -> Option<String> {
        index()
            .newest(&Version::parse(current).unwrap(), target, channel)
            .map(|available| available.version)
    }

    #[test]
    fn stable_channel_skips_prereleases() {
        assert_eq!(
            newest("0.1.0", MAC, UpdateChannel::Stable).as_deref(),
            Some("0.1.1")
        );
        assert_eq!(newest("0.1.1", MAC, UpdateChannel::Stable), None);
    }

    #[test]
    fn beta_channel_takes_the_newest_of_both() {
        assert_eq!(
            newest("0.1.1", MAC, UpdateChannel::Beta).as_deref(),
            Some("0.2.0-beta.1")
        );
        // 只有 arm64 的测试版不会提示给 Intel 机器
        assert_eq!(newest("0.1.1", MAC_INTEL, UpdateChannel::Beta), None);
    }

    #[test]
    fn drafts_never_count() {
        assert_eq!(
            newest("0.1.0", MAC, UpdateChannel::Beta).as_deref(),
            Some("0.2.0-beta.1")
        );
        assert!(
            index()
                .releases
                .iter()
                .all(|release| release.version != "9.9.9")
        );
    }

    #[test]
    fn universal_package_serves_both_cpus() {
        let release = index().releases.pop().unwrap();
        assert_eq!(release.version, "0.1.0");
        assert_eq!(release.assets.len(), 2);
    }

    #[test]
    fn notes_and_date_come_from_the_release() {
        let found = index()
            .newest(
                &Version::parse("0.1.0").unwrap(),
                MAC,
                UpdateChannel::Stable,
            )
            .unwrap();
        assert_eq!(found.date, "2026-10-10");
        assert_eq!(found.notes, ["修了 a", "加了 b"]);
    }

    #[test]
    fn invalid_json_is_an_error() {
        assert!(Index::parse(b"{}").is_err());
    }
}
