use serde::{Deserialize, Serialize};

/// 配置文件 `[dictionaries]` 分节：附加词库的开关。
///
/// 两类附加词库：随包的领域词库（`.app` 里 `Resources/dicts/`，法律 / 医学 / 地名 …）缺省关闭，列在 `domains` 里的才加载；
/// 用户目录 `dicts/` 下的 `.qj`（自己导入的）文件在就加载，只有列在 `disabled` 里的（按文件名，不含扩展名）跳过；
/// 导入 / 移除就是加 / 删文件。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DictionariesConfig {
    /// 打开的随包领域词库（文件名，不含 `.qj`）。
    pub domains: Vec<String>,

    /// 关掉的用户词库（文件名，不含 `.qj`）。
    pub disabled: Vec<String>,
}

/// 缺省打开的随包领域词库：成语四字全拼几乎不歧义，收益稳；其余按需打开。
pub const DEFAULT_DOMAINS: [&str; 1] = ["idioms"];

/// 随包词库里始终默认打开的：基础词库补缺的现代词（筋膜枪、内卷这类），不该要用户去勾；要关走 `disabled`。
pub const ALWAYS_ON_DOMAINS: [&str; 1] = ["modern"];

impl Default for DictionariesConfig {
    fn default() -> Self {
        Self {
            domains: DEFAULT_DOMAINS.iter().map(|s| (*s).to_owned()).collect(),
            disabled: Vec::new(),
        }
    }
}

impl DictionariesConfig {
    /// 用户目录里的词库是否启用。
    pub fn is_enabled(&self, stem: &str) -> bool {
        !self.disabled.iter().any(|d| d == stem)
    }

    /// 随包领域词库是否启用。
    pub fn is_domain_enabled(&self, stem: &str) -> bool {
        if Self::is_always_on(stem) {
            return self.is_enabled(stem);
        }
        self.domains.iter().any(|d| d == stem)
    }

    /// 这本随包词库是不是缺省就开（开关记在 `disabled` 而不是 `domains`）。
    pub fn is_always_on(stem: &str) -> bool {
        ALWAYS_ON_DOMAINS.contains(&stem)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modern_is_on_unless_disabled_and_other_domains_follow_the_list() {
        let mut config = DictionariesConfig {
            domains: vec!["idioms".to_owned()],
            disabled: Vec::new(),
        };
        assert!(config.is_domain_enabled("modern"));
        assert!(config.is_domain_enabled("idioms"));
        assert!(!config.is_domain_enabled("medicine"));
        config.disabled.push("modern".to_owned());
        assert!(!config.is_domain_enabled("modern"));
    }
}
