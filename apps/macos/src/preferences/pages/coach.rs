//! 「双语教练」页：总开关、复制解码 / 上屏组句两个触发、后端与它的模型 / 路径 / 地址 / 密钥、测试连接。

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{NSButton, NSPopUpButton, NSSecureTextField, NSTextField};
use objc2_foundation::NSString;
use subtext_coach::BackendKind;
use subtext_platform::Config;

use crate::preferences::controls::{
    button, checkbox, note, row_checkbox, row_control, row_popup, secure_field, select,
    set_checked, text_field,
};
use crate::preferences::layout::{Layout, PAGE_PADDING, ROW_HEIGHT};
use crate::preferences::setting::Setting;
use crate::preferences::target::PreferencesTarget;

pub struct CoachPage {
    enabled: Retained<NSButton>,

    auto_decode: Retained<NSButton>,

    auto_compose: Retained<NSButton>,

    auto_edit: Retained<NSButton>,

    backend: Retained<NSPopUpButton>,

    /// 模型：随后端变，显示当前后端那一项。
    model: Retained<NSTextField>,

    /// `claude` / `codex` 的路径，只有命令行后端用。
    path: Retained<NSTextField>,

    /// OpenAI 兼容接口地址，只有 openai 后端用。
    base_url: Retained<NSTextField>,

    /// 密钥输入框，永远不回显已有值；只有 API 后端用。
    api_key: Retained<NSSecureTextField>,

    test: Retained<NSButton>,
}

impl CoachPage {
    pub fn build(layout: &mut Layout, mtm: MainThreadMarker, target: &PreferencesTarget) -> Self {
        let enabled = checkbox(mtm, "启用双语教练", Setting::CoachEnabled, target);
        row_checkbox(layout, &enabled);
        note(
            layout,
            mtm,
            "复制了一段英文，就自动解码：语气、潜台词、俚语和缩写，译文先折叠，让你先自己猜；打完一句中文停一下，就给出地道的英文并说明为什么这样说。只在言外是当前输入法时工作。",
        );
        let auto_decode = checkbox(mtm, "复制英文时自动解码", Setting::CoachAutoDecode, target);
        row_checkbox(layout, &auto_decode);
        let auto_compose = checkbox(
            mtm,
            "打完中文自动给出英文",
            Setting::CoachAutoCompose,
            target,
        );
        row_checkbox(layout, &auto_compose);
        let auto_edit = checkbox(mtm, "打完英文自动校对", Setting::CoachAutoEdit, target);
        row_checkbox(layout, &auto_edit);
        note(
            layout,
            mtm,
            "开启后，复制的英文和上屏的中文会发给下面选的后端。密码框、密码管理器、疑似密钥或长文本不会发送。出结果后按 ⌥1 / ⌥2 / ⌥3 用对应的英文替换刚打的文字，Esc 关闭。打完英文停一下，会给出中文意思（核对有没有打错、听错）、改错和更地道的说法。",
        );
        let backends: Vec<String> = BackendKind::ALL
            .iter()
            .map(|kind| kind.label().to_owned())
            .collect();
        let backend = row_popup(
            layout,
            mtm,
            "后端",
            &backends,
            Setting::CoachBackend,
            target,
        );
        note(
            layout,
            mtm,
            "本机 Claude Code / Codex 走你自己的订阅额度，不需要密钥，要先在终端里登录过；API 后端按量计费。",
        );
        let model = text_field(mtm, Setting::CoachModel, target);
        row_control(layout, mtm, "模型", &model);
        let path = text_field(mtm, Setting::CoachPath, target);
        row_control(layout, mtm, "命令路径", &path);
        let base_url = text_field(mtm, Setting::CoachBaseUrl, target);
        row_control(layout, mtm, "接口地址", &base_url);
        let api_key = secure_field(mtm, Setting::CoachApiKey, target);
        row_control(layout, mtm, "API 密钥", &api_key);
        note(
            layout,
            mtm,
            "文本框按回车保存。命令路径留空则自动查找；密钥只保存在这台电脑上，不会随配置文件导出，也不显示已填的值。",
        );
        let test = button(mtm, "测试连接", Setting::CoachTest, target);
        layout.place(&test, PAGE_PADDING, 120.0, ROW_HEIGHT + 4.0);
        layout.next_row(ROW_HEIGHT + 4.0);
        note(
            layout,
            mtm,
            "用上面的设置发一条最小请求，结果显示在窗口底部。命令行后端第一次可能要十几秒。",
        );
        Self {
            enabled,
            auto_decode,
            auto_compose,
            auto_edit,
            backend,
            model,
            path,
            base_url,
            api_key,
            test,
        }
    }

    /// `key_present` 是当前后端的密钥已经有了（环境或配置里）；各行按后端启用 / 灰掉。
    pub fn sync(&self, config: &Config, key_present: bool) {
        let coach = &config.coach;
        set_checked(&self.enabled, coach.enabled);
        set_checked(&self.auto_decode, coach.auto_decode);
        set_checked(&self.auto_compose, coach.auto_compose);
        set_checked(&self.auto_edit, coach.auto_edit);
        for control in [&self.auto_decode, &self.auto_compose, &self.auto_edit] {
            control.setEnabled(coach.enabled);
        }
        select(
            &self.backend,
            BackendKind::ALL
                .iter()
                .position(|kind| *kind == coach.backend),
        );
        let (model, path) = match coach.backend {
            BackendKind::ClaudeCli => (&coach.claude_model, Some(&coach.claude_path)),
            BackendKind::CodexCli => (&coach.codex_model, Some(&coach.codex_path)),
            BackendKind::OpenAi => (&coach.openai_model, None),
            BackendKind::Anthropic => (&coach.anthropic_model, None),
        };
        self.model.setStringValue(&NSString::from_str(model));
        self.path
            .setStringValue(&NSString::from_str(path.map_or("", String::as_str)));
        self.path.setEnabled(path.is_some());
        self.base_url
            .setStringValue(&NSString::from_str(&coach.openai_base_url));
        self.base_url
            .setEnabled(coach.backend == BackendKind::OpenAi);
        let needs_key = !coach.backend.is_local_cli();
        self.api_key.setEnabled(needs_key);
        self.api_key.setStringValue(&NSString::from_str(""));
        let hint = match (needs_key, key_present) {
            (false, _) => "本机命令行不需要密钥",
            (true, true) => "已设置，输入新值可替换",
            (true, false) => "未设置",
        };
        self.api_key
            .setPlaceholderString(Some(&NSString::from_str(hint)));
        self.test.setEnabled(true);
    }
}
