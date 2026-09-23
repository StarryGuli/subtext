//! 悬浮状态条随模式、双拼方案与开关变化。

use qingjian_platform::protocol::IndicatorState;

use crate::support::*;

#[test]
fn status_bar_mode_click_is_handed_to_dll_via_sync_mode() {
    let config = RouterConfig {
        status_enabled: true,
        ..RouterConfig::default()
    };
    let mut router = router_with(config);
    let recorder = RecordingStatus::default();
    router.set_status_sink(Box::new(recorder.clone()));
    router.handle(ClientMessage::ModeChanged {
        session: SESSION,
        english: false,
    });

    // 点「中」：状态条先翻成「英」，DLL 来取时拿到目标模式，取一次就清。
    router.handle_status_event(StatusEvent::ToggleMode);
    assert_eq!(recorder.calls().last(), Some(&Some("英".to_owned())));
    assert_eq!(
        router.handle(ClientMessage::SyncMode { session: SESSION }),
        Some(ServerMessage::ModeSync {
            session: SESSION,
            english: Some(true),
            input: InputSettings::default(),
            indicator: IndicatorState {
                full_width_punctuation: true,
                english_full_width_punctuation: false,
                status_bar: true,
            },
        })
    );
    assert_eq!(
        router.handle(ClientMessage::SyncMode { session: SESSION }),
        Some(ServerMessage::ModeSync {
            session: SESSION,
            english: None,
            input: InputSettings::default(),
            indicator: IndicatorState {
                full_width_punctuation: true,
                english_full_width_punctuation: false,
                status_bar: true,
            },
        })
    );
}

#[test]
fn status_bar_mode_click_is_ignored_when_builtin_english_is_off() {
    let config = RouterConfig {
        status_enabled: true,
        english_mode: false,
        ..RouterConfig::default()
    };
    let mut router = router_with(config);
    let recorder = RecordingStatus::default();
    router.set_status_sink(Box::new(recorder.clone()));
    router.handle(ClientMessage::ModeChanged {
        session: SESSION,
        english: false,
    });
    assert_eq!(recorder.calls().last(), Some(&Some("中".to_owned())));

    // 关掉内置英文模式：点「中」不翻成「英」，也不给 DLL 递目标模式（DLL 那边同样会拦）
    router.handle_status_event(StatusEvent::ToggleMode);
    assert_eq!(recorder.calls().last(), Some(&Some("中".to_owned())));
    assert_eq!(
        router.handle(ClientMessage::SyncMode { session: SESSION }),
        Some(ServerMessage::ModeSync {
            session: SESSION,
            english: None,
            input: InputSettings {
                english_mode: false,
                ..InputSettings::default()
            },
            indicator: IndicatorState {
                full_width_punctuation: true,
                english_full_width_punctuation: false,
                status_bar: true,
            },
        })
    );
}

#[test]
fn status_bar_follows_mode_when_enabled() {
    let config = RouterConfig {
        status_enabled: true,
        ..RouterConfig::default()
    };
    let mut router = router_with(config);
    let recorder = RecordingStatus::default();
    router.set_status_sink(Box::new(recorder.clone()));

    // 中文 → 英文：各刷一次；会话关掉（应用退出）不收；切成别的输入法才收起。
    router.handle(ClientMessage::ModeChanged {
        session: SESSION,
        english: false,
    });
    router.handle(ClientMessage::ModeChanged {
        session: SESSION,
        english: true,
    });
    router.handle(ClientMessage::CloseSession { session: SESSION });
    assert_eq!(
        recorder.calls(),
        vec![Some("中".to_owned()), Some("英".to_owned())]
    );

    router.handle(ClientMessage::ImeSwitched { session: SESSION });
    assert_eq!(recorder.calls().last(), Some(&None));
}

#[test]
fn status_bar_shows_shuangpin_scheme_in_chinese() {
    let config = RouterConfig {
        status_enabled: true,
        scheme: Scheme::Shuangpin(ShuangpinScheme::Xiaohe),
        ..RouterConfig::default()
    };
    let mut router = router_with(config);
    let recorder = RecordingStatus::default();
    router.set_status_sink(Box::new(recorder.clone()));

    router.handle(ClientMessage::ModeChanged {
        session: SESSION,
        english: false,
    });

    assert_eq!(recorder.calls(), vec![Some("中 · 小鹤双拼".to_owned())]);
}

#[test]
fn status_bar_stays_hidden_when_disabled() {
    let mut router = router();
    let recorder = RecordingStatus::default();
    router.set_status_sink(Box::new(recorder.clone()));

    router.handle(ClientMessage::ModeChanged {
        session: SESSION,
        english: false,
    });

    assert_eq!(recorder.calls(), vec![None]);
}

#[test]
fn indicator_menu_toggles_status_bar() {
    use qingjian_platform::protocol::IndicatorCommand;

    let mut router = router();
    let recorder = RecordingStatus::default();
    router.set_status_sink(Box::new(recorder.clone()));
    router.handle(ClientMessage::ModeChanged {
        session: SESSION,
        english: false,
    });

    // 任务栏图标菜单里点「悬浮状态条」：关着的打开，再点收起。
    let toggle = ClientMessage::Indicator {
        session: SESSION,
        command: IndicatorCommand::ToggleStatusBar,
    };
    router.handle(toggle.clone());
    assert_eq!(recorder.calls().last(), Some(&Some("中".to_owned())));
    router.handle(toggle);
    assert_eq!(recorder.calls().last(), Some(&None));
}
