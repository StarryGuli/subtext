# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 仓库现状

Subtext（言外）：macOS 双语输入法 + AI 教练。输入法内核基于青简 Qingjian 二次开发，Core 平台无关，macOS 壳只做壳。只有 macOS 外壳。

## 目录地图

- `crates/subtext-core`：引擎。`Engine` 是对外唯一门面；拼音解析、纠错、候选、排序、整句、双拼、注音、英文模式。
- `crates/subtext-coach`：**双语教练**。`gate/` 外发闸门（语言、密钥、链接代码）、`prompt/` 三种模式的提示词、`backend/` 四个后端（claude-cli / codex-cli / openai / anthropic）、`service/` 后台线程与缓存；自带命令行 `subtext-coach`。
- `crates/subtext-coach/src/screen/`：屏幕阅读的平台无关部分：OCR 行合并成消息块（`block.rs`）、记住分析过的块并认出 OCR 抖动（`memory.rs`）。
- `crates/subtext-predict`：云联想 `CloudPredictor`（OpenAI 兼容接口）与释义兜底。
- `crates/subtext-dictionary` / `-translate` / `-learning` / `-lm` / `-neural` / `-format`：词库、释义表、用户侧落盘、语言模型、本地推理、`.qj` 数据容器。
- `crates/subtext-platform`：`Config`（TOML）、日志与密钥掩码、资源路径。`[coach]` 与 `[shortcut] coach_selection` 在这里。
- `crates/subtext-render`：候选窗自绘渲染器。
- `crates/subtext-update`：检查更新，读 GitHub Releases，只提示不安装。
- `apps/cli`：Core 的验证工具。
- `apps/macos`：IMK 壳，按 `app / host / imk / candidates / menubar / preferences / coach` 分目录。`coach/` 是教练的壳侧：触发、面板（`panel.rs` + `attributed.rs` + `doc.rs`）、替换；`host/coach.rs` 是借用 `Host` 之外碰应用客户端的胶水。`screen/` 是屏幕阅读的壳侧：窗口列表（`windows.rs`）、截屏与权限（`capture.rs`）、OCR 帮手调用（`ocr.rs`）、后台扫描线程（`scan.rs`）、状态机与悬浮卡（`reader.rs`）、框选区域（`region.rs`）、端到端自测（`selftest.rs`）。`scripts/bundle.sh --pkg` 出安装包。
- `tools/ocr/subtext-ocr.swift`：屏幕阅读的 OCR 帮手（Swift，系统 Vision），`bundle.sh` 按目标架构编译进包。
- `tools/dict-convert`、`tools/gloss-gen`、`tools/corpus`：产品数据生成，输出到 `data/generated/`（gitignore）。
- `assets/`：随包数据源、图标（`icon/`）、README 用的品牌图与截图（`brand/`、`screenshots/`）。

## 常用命令

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p subtext-coach -- "hey, no rush but did you look at the PR?"   # 直接试教练
cargo run -p subtext-coach -- --print-prompt "…"                          # 看实际发出去的提示词
apps/macos/scripts/bundle.sh --pkg                                        # 出 pkg（需要 data/generated/）
subtext-macos --coach-preview decode docs/demo/decode.json out.png dark   # 不装输入法预览面板
SUBTEXT_OCR_BIN=… subtext-macos --screen-selftest assets/screenshots/compose-light.png   # 屏幕阅读端到端自测（真 OCR + 假后端）
subtext-macos --screen-probe                                              # 打印屏幕录制权限与窗口列表
```

IMK 壳不能 `cargo run`，必须打包安装。**不要在没问用户的情况下 `--install` / `--register`**：那会改用户的系统输入源。

## 约定

架构约束、代码组织、版本号、提交信息、提交前检查都在 `docs/contributing.md`，随本文件一起载入：

@docs/contributing.md

交流用中文。
