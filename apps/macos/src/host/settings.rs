//! 菜单与偏好设置窗口的动作：只改 config.toml（或触发一次性操作），改完由 apply_config 统一生效。

use super::diagnostics::{copy_to_pasteboard, open_with_system};
use super::*;
use crate::preferences::DEFAULT_FONT_LABEL;
use subtext_platform::ShiftLetter;

impl Host {
    /// 写短语前读取文件；外部规则有变化时同步列表并请用户重新确认。
    fn phrases_are_current(&mut self) -> bool {
        let Some(path) = self.settings.path() else {
            return false;
        };
        match subtext_platform::Config::load(path) {
            Ok(latest) if latest.custom_phrases == self.settings.config().custom_phrases => true,
            Ok(_) => {
                self.settings.reload();
                self.apply_config(false);
                let message = "规则已在其他地方修改，请重新确认后操作。";
                self.preferences.set_phrase_error(message);
                self.preferences.set_status(message);
                false
            }
            Err(error) => {
                self.preferences.set_phrase_error(&error.to_string());
                self.preferences.set_status(&error.to_string());
                false
            }
        }
    }

    /// 表格中的启用开关只修改所选规则。
    pub fn set_phrase_enabled(&mut self, index: usize, enabled: bool) {
        if !self.phrases_are_current() {
            return;
        }
        let mut phrases = self.settings.config().custom_phrases.clone();
        let Some(phrase) = phrases.get_mut(index) else {
            return;
        };
        phrase.enabled = enabled;
        let Some(path) = self.settings.path() else {
            return;
        };
        let result = subtext_platform::Config::set_custom_phrases(path, &phrases);
        self.settings.reload();
        self.apply_config(false);
        if let Err(error) = result {
            self.preferences.set_status(&error);
        }
    }

    /// 菜单动作。开关类先落盘再热加载，菜单勾选状态永远来自文件里的值。
    pub fn perform(&mut self, action: MenuAction) {
        tracing::info!(?action, "菜单");
        match action {
            MenuAction::ToggleCloud => {
                let on = !self.settings.config().predict.enabled;
                if self.settings.set_bool("predict", "enabled", on) {
                    self.apply_config(false);
                }
            }
            MenuAction::ToggleCoach => {
                let on = !self.settings.config().coach.enabled;
                if self.settings.set_bool("coach", "enabled", on) {
                    self.apply_config(false);
                }
            }
            MenuAction::ToggleFuzzy(index) => {
                let name = FuzzyRules::NAMES[index];
                let on = !self.settings.config().fuzzy.is_on(name);
                if self.settings.set_bool("fuzzy", name, on) {
                    self.apply_config(false);
                }
            }
            MenuAction::ScreenReadWindow => {
                self.screen
                    .schedule_window_pick(std::time::Duration::from_secs(3));
                self.show_notice(
                    "3 秒后读取鼠标所在的窗口：请把鼠标移到要读的窗口上",
                    objc2_foundation::NSRect::ZERO,
                );
            }
            MenuAction::ScreenReadRegion => {
                self.screen.begin_region_pick();
                if let Some(text) = self.screen.take_notice() {
                    self.show_notice(&text, objc2_foundation::NSRect::ZERO);
                }
            }
            MenuAction::ScreenReadStop => {
                self.screen.stop();
                if let Some(text) = self.screen.take_notice() {
                    self.show_notice(&text, objc2_foundation::NSRect::ZERO);
                }
            }
            MenuAction::OpenFuzzySettings => {
                self.preferences.show_page("模糊音");
            }
            MenuAction::OpenPreferences => {
                self.preferences.sync_usage(
                    &self.engine.usage_summary(),
                    &self.engine.vocabulary_summary(),
                    self.learning_language,
                );
                self.preferences.show();
            }
            MenuAction::OpenLogs => {
                if let Some(dir) = logging::log_dir() {
                    open_with_system(&[&dir.to_string_lossy()]);
                }
            }
            MenuAction::OpenDownload => open_with_system(&[subtext_update::DOWNLOAD_URL]),
        }
    }

    /// 设置窗口里改了一个控件：写配置、热加载；写不成（非法组合、空文本）也要把控件同步回真实值。
    pub fn change_setting(&mut self, setting: Setting, value: SettingValue) {
        // 密钥值不进日志
        tracing::info!(?setting, "设置");
        let config = self.settings.config().clone();
        match (setting, value) {
            (Setting::NewPhrase, _) => {
                self.preferences.edit_phrase(&config, None);
                return;
            }
            (Setting::EditPhrase, _) => {
                if let Some(index) = self.preferences.selected_phrase() {
                    self.preferences.edit_phrase(&config, Some(index));
                }
                return;
            }
            (Setting::CancelPhraseEdit, _) => {
                self.preferences.close_phrase_editor();
                return;
            }

            (Setting::PhraseDraft, _) => return,
            (Setting::SelectPhrase, SettingValue::Index(index)) => {
                self.preferences.select_phrase(&config, index);
                return;
            }
            (Setting::SavePhrase | Setting::DeletePhrase, _) => {
                if !self.phrases_are_current() {
                    return;
                }
                let config = self.settings.config();
                let mut phrases = config.custom_phrases.clone();
                let mut saved_index = phrases.len();
                if setting == Setting::DeletePhrase {
                    let Some(index) = self.preferences.selected_phrase() else {
                        return;
                    };
                    if index >= phrases.len() {
                        return;
                    }
                    phrases.remove(index);
                } else {
                    let (index, draft) = match self.preferences.phrase_draft(config) {
                        Ok(value) => value,
                        Err(error) => {
                            self.preferences.set_phrase_error(&error);
                            self.preferences.set_status(&error);
                            return;
                        }
                    };
                    if let Some(index) = index {
                        let Some(phrase) = phrases.get_mut(index) else {
                            return;
                        };
                        *phrase = draft;
                        saved_index = index;
                    } else {
                        phrases.push(draft);
                    }
                }
                let Some(path) = self.settings.path() else {
                    return;
                };
                if let Err(error) = subtext_platform::Config::set_custom_phrases(path, &phrases) {
                    self.preferences.set_phrase_error(&error);
                    self.preferences.set_status(&error);
                    return;
                }
                self.preferences.close_phrase_editor();
                self.settings.reload();
                self.apply_config(false);
                if setting == Setting::SavePhrase {
                    self.preferences
                        .select_phrase(self.settings.config(), saved_index + 1);
                }
                self.preferences.set_status("自定义短语已保存");
                return;
            }
            (Setting::FullWidthPunctuation, SettingValue::Index(index)) => {
                self.settings
                    .set_bool("general", "full_width_punctuation", index == 0);
            }
            (Setting::LearningLanguage, SettingValue::Index(index)) => {
                // 菜单最后一项是「不显示译文」
                let code = self
                    .languages
                    .get(index)
                    .map_or(LEARNING_LANGUAGE_OFF, |language| language.code());
                self.settings
                    .set_value("general", "learning_language", code);
            }
            (Setting::PageSize, SettingValue::Index(index)) => {
                self.settings
                    .set_value("general", "page_size", index as i64 + 1);
            }
            (Setting::PageKeys, SettingValue::Index(index)) => {
                if let Some(pair) = PAGE_KEY_OPTIONS.get(index) {
                    self.settings.set_value("general", "page_keys", *pair);
                }
            }
            (Setting::Theme, SettingValue::Index(index)) => {
                if let Some(theme) = ThemeMode::ALL.get(index) {
                    self.settings.set_value("general", "theme", theme.key());
                }
            }
            (Setting::Renderer, SettingValue::Index(index)) => {
                if let Some(renderer) = CandidateRenderer::ALL.get(index) {
                    self.settings
                        .set_value("general", "renderer", renderer.key());
                }
            }
            (Setting::Font, SettingValue::Text(text)) => {
                let font = text.trim();
                let font = if font == DEFAULT_FONT_LABEL { "" } else { font };
                self.settings.set_value("general", "font", font);
            }
            (Setting::Layout, SettingValue::Index(index)) => {
                if let Some(layout) = LayoutMode::ALL.get(index) {
                    self.settings.set_value("general", "layout", layout.key());
                }
            }
            (Setting::HorizontalGrid, SettingValue::Bool(on)) => {
                self.settings.set_bool("general", "horizontal_grid", on);
            }
            (Setting::Preedit, SettingValue::Index(index)) => {
                if let Some(mode) = PreeditMode::ALL.get(index) {
                    self.settings.set_value("general", "preedit", mode.key());
                }
            }
            (Setting::QuestionMark, SettingValue::Bool(on)) => {
                self.settings.set_bool("shortcut", "question_mark", on);
            }
            (Setting::ExpressionKey | Setting::QuestionKey, SettingValue::Index(index)) => {
                if let Some(&key) = ModeKeys::CANDIDATES.get(index) {
                    let mut keys = config.shortcut.mode.sanitized();
                    let name = if setting == Setting::ExpressionKey {
                        keys.expression = key;
                        "expression"
                    } else {
                        keys.question = key;
                        "question"
                    };
                    if keys.is_valid() {
                        self.settings.set_value("shortcut", name, key.to_string());
                    } else {
                        tracing::warn!("表达式键与问字键不能相同，未改");
                    }
                }
            }
            (
                Setting::TranslationKeys | Setting::TranslationSecondKeys,
                SettingValue::Text(text),
            ) => match text.parse::<Modifiers>() {
                Ok(chosen) => {
                    let (first, second) = config.shortcut.translation_keys();
                    let (name, other) = if setting == Setting::TranslationKeys {
                        ("translation", second)
                    } else {
                        ("translation_second", first)
                    };
                    if chosen == other {
                        tracing::warn!("两组译词快捷键不能相同，未改");
                    } else {
                        self.settings.set_value("shortcut", name, chosen.key());
                    }
                }
                Err(error) => tracing::warn!(%error, "修饰键组合不合法，未改"),
            },
            (Setting::DeleteCandidateKeys, SettingValue::Text(text)) => {
                match text.parse::<Modifiers>() {
                    Ok(chosen) => {
                        let (first, second) = config.shortcut.translation_keys();
                        if chosen == first || chosen == second {
                            tracing::warn!("删候选的快捷键不能与译词快捷键相同，未改");
                        } else {
                            self.settings
                                .set_value("shortcut", "delete_candidate", chosen.key());
                        }
                    }
                    Err(error) => tracing::warn!(%error, "修饰键组合不合法，未改"),
                }
            }
            (Setting::TranslateSelectionKeys, SettingValue::Text(text)) => {
                match text.parse::<KeyCombo>() {
                    Ok(combo) => {
                        self.settings.set_value(
                            "shortcut",
                            "translate_selection",
                            combo.key_string(),
                        );
                    }
                    Err(error) => tracing::warn!(%error, "快捷键不合法，未改"),
                }
            }
            (Setting::ResetShortcuts, _) => {
                let defaults = ShortcutConfig::default();
                self.settings
                    .set_value("general", "page_keys", PAGE_KEY_OPTIONS[0]);
                self.settings.set_value(
                    "shortcut",
                    "expression",
                    defaults.mode.expression.to_string(),
                );
                self.settings
                    .set_value("shortcut", "question", defaults.mode.question.to_string());
                self.settings
                    .set_bool("shortcut", "question_mark", defaults.mode.question_mark);
                self.settings
                    .set_value("shortcut", "translation", defaults.translation.key());
                self.settings.set_value(
                    "shortcut",
                    "translation_second",
                    defaults.translation_second.key(),
                );
                self.settings.set_value(
                    "shortcut",
                    "translate_selection",
                    defaults.translate_selection.key_string(),
                );
                self.settings.set_value(
                    "shortcut",
                    "delete_candidate",
                    defaults.delete_candidate.key(),
                );
            }
            (Setting::DictionaryEnabled(index), SettingValue::Bool(on)) => {
                if let Some(info) = self.dictionary_list.get(index).cloned() {
                    if info.builtin {
                        self.set_domain_enabled(&info.stem, on);
                    } else {
                        self.set_dictionary_enabled(&info.stem, on);
                    }
                }
            }
            (Setting::DictionaryRemove(index), _) => {
                self.remove_dictionary(index);
                return;
            }
            (Setting::ImportDictionary, _) => {
                if let Some(path) = crate::preferences::choose_dictionary_file() {
                    self.import_dictionary(&path);
                }
                return;
            }
            (Setting::Fuzzy(index), SettingValue::Bool(on)) => {
                self.settings
                    .set_bool("fuzzy", FuzzyRules::NAMES[index], on);
            }
            (Setting::CloudEnabled, SettingValue::Bool(on)) => {
                self.settings.set_bool("predict", "enabled", on);
            }
            (Setting::LocalModelEnabled, SettingValue::Bool(on)) => {
                self.settings.set_bool("model", "enabled", on);
            }
            (Setting::UpdateCheck, SettingValue::Bool(on)) => {
                self.settings.set_bool("update", "check", on);
            }
            (Setting::UpdateChannel, SettingValue::Index(index)) => {
                if let Some(channel) = UpdateChannel::ALL.get(index) {
                    self.settings.set_value("update", "channel", channel.key());
                }
            }
            (Setting::CloudSlots, SettingValue::Index(index)) => {
                self.settings.set_value("predict", "slots", index as i64);
            }
            (Setting::Traditional, SettingValue::Bool(on)) => {
                self.settings.set_bool("general", "traditional", on);
            }
            (Setting::EnglishCandidates, SettingValue::Bool(on)) => {
                self.settings.set_bool("general", "english_candidates", on);
            }
            (Setting::ChineseFirst, SettingValue::Bool(on)) => {
                self.settings.set_bool("general", "chinese_first", on);
            }
            (Setting::ShiftLetter, SettingValue::Bool(on)) => {
                let mode = if on {
                    ShiftLetter::Compose
                } else {
                    ShiftLetter::Passthrough
                };
                self.settings
                    .set_value("general", "shift_letter", mode.key());
            }
            // 勾上写缺省的终端 / 编辑器列表，去掉写空表；手改过的列表勾一下就回缺省
            (Setting::EnglishCandidatesOffInApps, SettingValue::Bool(on)) => {
                let apps: toml_edit::Array = if on {
                    DEFAULT_ENGLISH_CANDIDATES_OFF.iter().copied().collect()
                } else {
                    toml_edit::Array::new()
                };
                self.settings
                    .set_value("apps", "english_candidates_off", apps);
            }
            // 弹出菜单按 Scheme::ALL 的顺序。写的是 [general] scheme（旧键 shuangpin 已并入它）：
            // 写旧键的话，配置里 scheme 的缺省值非空、解析时优先，用户选的方案会被静默忽略。
            (Setting::Scheme, SettingValue::Index(index)) => {
                let key = Scheme::ALL
                    .get(index)
                    .map_or(Scheme::Pinyin.key(), |scheme| scheme.key());
                self.settings.set_value("general", "scheme", key);
            }
            (Setting::ShuangpinRawPreedit, SettingValue::Bool(on)) => {
                self.settings
                    .set_bool("general", "shuangpin_raw_preedit", on);
            }
            // 五笔：勾上就是 86 版，取消就是关。与上面的拼音方案同时开着就是混输。
            (Setting::Wubi, SettingValue::Bool(on)) => {
                self.settings
                    .set_value("general", "wubi", if on { "wubi86" } else { "" });
            }
            (Setting::CoachEnabled, SettingValue::Bool(on)) => {
                self.settings.set_bool("coach", "enabled", on);
            }
            (Setting::CoachAutoDecode, SettingValue::Bool(on)) => {
                self.settings.set_bool("coach", "auto_decode", on);
            }
            (Setting::CoachAutoCompose, SettingValue::Bool(on)) => {
                self.settings.set_bool("coach", "auto_compose", on);
            }
            (Setting::CoachAutoEdit, SettingValue::Bool(on)) => {
                self.settings.set_bool("coach", "auto_edit", on);
            }
            (Setting::CoachBackend, SettingValue::Index(index)) => {
                if let Some(kind) = subtext_coach::BackendKind::ALL.get(index) {
                    self.settings.set_value("coach", "backend", kind.key());
                }
            }
            (Setting::CoachModel, SettingValue::Text(text)) => {
                let text = text.trim();
                let (key, current) = coach_model_field(&config.coach);
                if !text.is_empty() && text != current {
                    self.settings.set_value("coach", key, text);
                }
            }
            (Setting::CoachPath, SettingValue::Text(text)) => {
                let text = text.trim();
                if let Some((key, current)) = coach_path_field(&config.coach)
                    && text != current
                {
                    self.settings.set_value("coach", key, text);
                }
            }
            (Setting::CoachBaseUrl, SettingValue::Text(text)) => {
                let text = text.trim();
                if !text.is_empty() && text != config.coach.openai_base_url {
                    self.settings.set_value("coach", "openai_base_url", text);
                }
            }
            (Setting::CoachApiKey, SettingValue::Text(text)) => {
                let text = text.trim();
                if text.chars().any(|c| !c.is_ascii_graphic()) {
                    self.preferences.set_status(
                        "密钥没有保存：里面有空格或非英文字符，多半是粘贴时多带了别的内容",
                    );
                    return;
                }
                let Some(env_name) = coach_key_env(&config.coach) else {
                    return;
                };
                if text.is_empty() {
                    return;
                }
                if self.settings.set_env_var(env_name, text) {
                    // 后端在建的时候读密钥，换了密钥必须重建
                    self.coach.restart(&config.coach);
                    self.preferences.set_status("密钥已保存");
                } else {
                    self.preferences
                        .set_status("密钥没有保存：写不进配置目录的 .env，详情见日志");
                }
                return;
            }
            (Setting::CoachTest, _) => {
                self.preferences.set_status("正在测试双语教练连接…");
                self.coach.start_test(&config.coach);
                return;
            }
            // 文本框失焦也会发 action：值没变就不写，免得每次切窗口都重写一遍配置
            (Setting::BaseUrl, SettingValue::Text(text)) => {
                let text = text.trim();
                if !text.is_empty() && text != config.predict.base_url {
                    self.settings.set_value("predict", "base_url", text);
                }
            }
            (Setting::Model, SettingValue::Text(text)) => {
                let text = text.trim();
                if !text.is_empty() && text != config.predict.model {
                    self.settings.set_value("predict", "model", text);
                }
            }
            (Setting::ApiKey, SettingValue::Text(text)) => {
                let text = text.trim();
                // 密码框看不见内容，粘贴多了（带上了终端提示符、命令）用户发现不了；这种值写进 .env 还会让整个文件解析失败
                if text.chars().any(|c| !c.is_ascii_graphic()) {
                    self.preferences.set_status(
                        "密钥没有保存：里面有空格或非英文字符，多半是粘贴时多带了别的内容",
                    );
                    return;
                }
                if text.is_empty() {
                    return;
                }
                if self.settings.set_env_var(&config.predict.api_key_env, text) {
                    // 密钥换了必须重建 Predictor
                    self.apply_config(true);
                    self.preferences.set_status("密钥已保存");
                } else {
                    self.preferences
                        .set_status("密钥没有保存：写不进配置目录的 .env，详情见日志");
                }
                return;
            }
            (Setting::TestCloud, _) => {
                self.start_cloud_test();
                return;
            }
            (Setting::OpenConfigFile, _) => {
                if let Some(path) = self.settings.path() {
                    open_with_system(&["-t", &path.to_string_lossy()]);
                }
                return;
            }
            (Setting::InputLog, SettingValue::Bool(on)) => {
                self.settings.set_bool("general", "input_log", on);
            }
            (Setting::Learning, SettingValue::Bool(on)) => {
                self.settings.set_bool("general", "learning", on);
            }
            (Setting::SystemTextReplacements, SettingValue::Bool(on)) => {
                self.settings
                    .set_bool("general", "system_text_replacements", on);
            }
            (Setting::ClearInputLog, _) => {
                self.clear_input_log();
                return;
            }
            (Setting::VerboseLog, SettingValue::Bool(on)) => {
                let level = if on { LogLevel::Debug } else { LogLevel::Info };
                self.settings.set_value("general", "log_level", level.key());
            }
            (Setting::CheckUpdateNow, _) => {
                if let Some(updates) = &self.updates {
                    updates.check_now(&self.settings.config().update);
                }
                self.sync_update();
                return;
            }
            (Setting::OpenDownload, _) => {
                open_with_system(&[subtext_update::DOWNLOAD_URL]);
                return;
            }
            (Setting::OpenWebsite, _) => {
                open_with_system(&[crate::preferences::WEBSITE_URL]);
                return;
            }
            (Setting::OpenRepository, _) => {
                open_with_system(&[crate::preferences::REPOSITORY_URL]);
                return;
            }
            (Setting::OpenLogDirectory, _) => {
                if let Some(dir) = logging::log_dir() {
                    open_with_system(&[&dir.to_string_lossy()]);
                }
                return;
            }
            (Setting::CopyDiagnostics, _) => {
                copy_to_pasteboard(&self.diagnostics());
                self.preferences
                    .set_status("诊断信息已复制到剪贴板，粘贴给作者即可");
                return;
            }
            (Setting::ExportLogs, _) => {
                match logging::export_logs() {
                    Ok(zip) => {
                        open_with_system(&["-R", &zip.to_string_lossy()]);
                        self.preferences
                            .set_status("日志已打包到桌面，发给作者即可");
                    }
                    Err(error) => self
                        .preferences
                        .set_status(&format!("打包日志失败：{error}")),
                }
                return;
            }
            (setting, value) => tracing::warn!(?setting, ?value, "设置项与控件值不匹配"),
        }
        self.apply_config(false);
    }
}

/// 当前后端的模型对应哪个配置键，以及它现在的值。
fn coach_model_field(coach: &subtext_coach::CoachConfig) -> (&'static str, &str) {
    use subtext_coach::BackendKind;
    match coach.backend {
        BackendKind::ClaudeCli => ("claude_model", &coach.claude_model),
        BackendKind::CodexCli => ("codex_model", &coach.codex_model),
        BackendKind::OpenAi => ("openai_model", &coach.openai_model),
        BackendKind::Anthropic => ("anthropic_model", &coach.anthropic_model),
    }
}

/// 命令行后端的路径对应哪个配置键；API 后端没有路径。
fn coach_path_field(coach: &subtext_coach::CoachConfig) -> Option<(&'static str, &str)> {
    use subtext_coach::BackendKind;
    match coach.backend {
        BackendKind::ClaudeCli => Some(("claude_path", &coach.claude_path)),
        BackendKind::CodexCli => Some(("codex_path", &coach.codex_path)),
        BackendKind::OpenAi | BackendKind::Anthropic => None,
    }
}

/// API 后端的密钥存在哪个环境变量里；命令行后端不需要密钥。
fn coach_key_env(coach: &subtext_coach::CoachConfig) -> Option<&str> {
    use subtext_coach::BackendKind;
    match coach.backend {
        BackendKind::OpenAi => Some(&coach.openai_api_key_env),
        BackendKind::Anthropic => Some(&coach.anthropic_api_key_env),
        BackendKind::ClaudeCli | BackendKind::CodexCli => None,
    }
}
