# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 仓库现状

Subtext（言外）：macOS 菜单栏英文教练，读 / 写 / 改三种模式手动选。0.4 起不再是输入法（旧版见 `v0.3.6` 标签）。

## 目录地图

- `crates/subtext-coach`：**教练**。`gate/` 外发闸门（语言、密钥、链接代码；手动提交走 `check_manual`）、`prompt/` 各模式提示词、`backend/` 四个后端（claude-cli / codex-cli / openai / anthropic）、`service/` 后台线程与缓存、`output/` 输出解析；自带命令行 `subtext-coach`。`screen/` 是输入法时代屏幕阅读的平台无关部分，目前没有应用使用。
- `apps/coach`：macOS 应用（包名 `subtext-app`，二进制 `subtext`）。`window.rs` 窗口与菜单栏图标、`hotkey.rs` 全局热键（Carbon）、`doc.rs` + `attributed.rs` 把结果排成富文本、`config.rs` 读配置、`scripts/bundle.sh` 出 `.app`。
- `tools/ocr`：输入法时代的 OCR 帮手（Swift），目前没有应用使用。
- `assets/`：图标（`icon/`）、README 用的品牌图与截图（`brand/`、`screenshots/`；截图是输入法时代的，待更新）。

## 常用命令

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p subtext-coach -- --mode decode "hey, no rush but did you look at the PR?"   # 直接试教练
cargo run -p subtext-coach -- --print-prompt "…"                                         # 看实际发出去的提示词
apps/coach/scripts/bundle.sh                                                              # 出 target/app/Subtext.app
target/debug/subtext --snapshot decode docs/demo/decode.json "输入文字" out.png            # 不要屏幕权限，把窗口渲染成 PNG
```

## 约定

架构约束、代码组织、版本号、提交信息、提交前检查都在 `docs/contributing.md`，随本文件一起载入：

@docs/contributing.md

交流用中文。
