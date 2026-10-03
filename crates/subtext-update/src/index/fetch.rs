use std::time::Duration;

use super::Index;
use crate::UpdateError;

/// 版本列表：GitHub Releases 接口；`SUBTEXT_UPDATE_INDEX` 可以换成别的地址（测试用）。
const INDEX_URL: &str = "https://api.github.com/repos/StarryGuli/subtext/releases?per_page=30";

const MAX_INDEX_BYTES: usize = 4 * 1024 * 1024;

const TIMEOUT: Duration = Duration::from_secs(20);

/// 下载版本列表并解析。请求只带 `subtext/<版本>` 的 User-Agent，没有任何标识。
pub(crate) fn fetch_index(current_version: &str) -> Result<Index, UpdateError> {
    let url = std::env::var("SUBTEXT_UPDATE_INDEX").unwrap_or_else(|_| INDEX_URL.to_owned());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async {
        let client = reqwest::Client::builder()
            .timeout(TIMEOUT)
            .user_agent(format!("subtext/{current_version}"))
            .build()?;
        let response = client
            .get(&url)
            .header("Accept", "application/vnd.github+json")
            .send()
            .await?
            .error_for_status()?;
        let bytes = response.bytes().await?;
        if bytes.len() > MAX_INDEX_BYTES {
            return Err(UpdateError::TooLarge(MAX_INDEX_BYTES));
        }
        Index::parse(&bytes)
    })
}
