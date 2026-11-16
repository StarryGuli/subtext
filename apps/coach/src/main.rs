//! 言外：菜单栏里的英文教练。读、写、改三种模式手动选，没有输入法。

mod attributed;
mod config;
mod doc;
mod edit_menu;
mod hotkey;
mod window;

use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let mtm = MainThreadMarker::new().expect("必须在主线程启动");
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    edit_menu::install(mtm);
    window::init(mtm, &config::load());
    if let Some(code) = window::snapshot_if_requested() {
        std::process::exit(code);
    }
    if !hotkey::register(window::present) {
        tracing::warn!("快捷键 {} 没注册上，只能从菜单栏图标打开", hotkey::LABEL);
    }
    window::present();
    app.run();
}
