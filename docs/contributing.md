# 开发约定

改代码前先看这一页：架构上不能越的线、代码怎么组织、版本号与提交信息怎么写。
设计的来龙去脉在 [design/architecture.md](design/architecture.md) 与 [design/coach.md](design/coach.md)。

## 架构约束（不要违反）

- **Core 与平台层严格解耦。** `subtext-core` 及其兄弟 crate（含 `subtext-coach`）必须平台无关：词库、拼音解析、候选生成、排序、学习、翻译、
  教练的闸门 / 提示词 / 后端全部属于它们。平台层（IMK 壳）只做两件事：把系统事件翻译成 Core 的输入，把结果画出来。
  **壳里不允许出现排序逻辑、词库访问、提示词、网络请求。** 壳里与教练有关的只有：触发（剪贴板轮询、上屏停顿）、面板、替换应用里的文字。
- **输入优先于一切附加功能。** 任何为学习或教练增加的延迟、弹窗、UI 干扰都是设计错误。教练的请求在后台线程里发，
  主线程只往通道里丢请求、取事件；Core 必须能在教练 / 译文没就绪时先返回候选。
- **外发必须过闸门。** 凡是会把用户的文字发往后端的路径，一律先过 `subtext_coach::gate::Gate`；新增触发源要补闸门测试。
  闸门宁可多拦：漏拦的代价远大于误拦。
- **一个候选词只显示一种辅助语言。** 译文是候选词的 annotation（可选、单条），不是并列的第二套候选系统。
- **碰应用客户端的步骤放在借用 `Host` 之外。** IMK 在等应用回话时会跑 run loop，重入回调会在 `host::with` 里借用失败；
  见 `host/mod.rs` 里 `with` 的说明与 `host/coach.rs` 的做法。
- **显示面自绘、控件面原生。** 候选窗、拼音行由渲染器出位图；偏好设置、菜单用 AppKit 原生控件；教练面板是原生 NSPanel + 富文本。

## 代码组织

- **一个类型一个文件。** 一个 struct / enum / trait 及其 impl 单独一个文件；模块文件只做 `mod` 声明、re-export 与自由函数。
- **子模块用目录。** 有子模块的模块用 `foo/mod.rs`，不用 `foo.rs` + `foo/` 并列。
- **同词干的兄弟文件收进目录，不用文件名前缀分组。**
- **大类型的 `impl` 按职责拆成子模块。** 每个文件一个 `impl Foo { … }`，结构体与构造留在 `mod.rs`，跨文件用到的私有方法标 `pub(super)`。
- **文件长度。** 单文件不超过 800 行，目标 500 行以内；测试超过 200 行搬到 `tests.rs`。
- **导入写精确路径。** 不用 `use foo::*`（`#[cfg(test)] mod tests` 里的 `use super::*` 除外）。

## 命名与注释

- 代码标识符一律英文，注释与文档用中文；`thiserror` 的 `#[error]` 文案用英文，日志与 UI 文案用中文。
- 新文件都要有 `//!` 文件头。注释只写维护时用得上的：文件头一两句说它是什么，代码里只解释读代码看不出来的约束与原因。
  不写复述代码的教学性注释，不写改动经过。不写装饰性分隔注释（`// ====`）。

## 依赖与配置

- 依赖用 `cargo add` 加，共用包提到根 `[workspace.dependencies]`；错误用 `thiserror` 不用 `anyhow`；日志用 `tracing` 门面。
- 快捷键一律进 `[shortcut]` 可配置，不写死键码。
- 密钥只进 `.env`，不进 `config.toml`，不进日志（`subtext_platform::logs::secrets::register` 登记后日志会掩码）。

## 版本号

- `crates/*` 用 `version.workspace = true`；`apps/macos` 是独立发布的产品，写死自己的 `version`。
- 发版之间带 `-dev`，打包脚本再接 git 短哈希；发版提交去掉 `-dev` 打标签 `v<版本>`，标签后再改成下一个 `-dev`。
- 改了 `apps/macos/Cargo.toml` 的版本后用 `cargo metadata --offline` 同步 `Cargo.lock`，否则 `bundle.sh` 的 `--locked` 会报错。

## 提交信息

[Conventional Commits](https://www.conventionalcommits.org/zh-hans/)：第一行 `<类型>(<范围>): <说明>`，类型与范围英文小写，说明用中文。

- 类型：`feat` / `fix` / `docs` / `refactor` / `perf` / `test` / `build` / `ci` / `chore` / `style` / `revert`。
- 范围：crate 或壳的名字——`core` `platform` `render` `dictionary` `translate` `learning` `predict` `coach` `lm` `neural` `format` `update` `cli` `macos` `tools` `docs` `ci` `deps` `brand` `release`。
- 正文写「为什么」与取舍，一行一条；不加 AI 署名。

## 提交前检查

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

`.githooks/` 里有对应的钩子，`git config core.hooksPath .githooks` 启用一次。
排序 / 整句 / 纠错的改动先跑 `apps/cli` 再合；教练提示词的改动用 `subtext-coach --print-prompt` 看实际发出去的内容，用真实模型的回复文件配 `--from-reply` 验证解析。

## 发版

1. 改 `apps/macos/Cargo.toml` 去掉 `-dev`，同步 `Cargo.lock`，更新 `CHANGELOG.md`。
2. `apps/macos/scripts/bundle.sh --pkg` 出 `target/pkg/subtext-<版本>-macos-<arch>.pkg`（需要 `data/generated/` 里的产品数据，见 README）。
3. 打标签 `v<版本>`，在 GitHub 上建 Release，附上 pkg 与 `SHA256SUMS`。有 Developer ID 证书时设 `SUBTEXT_SIGN_IDENTITY` /
   `SUBTEXT_INSTALLER_IDENTITY` / `SUBTEXT_NOTARY_PROFILE` 再打包（见 `apps/macos/README.md`）；没有就是 ad-hoc 签名，Release 说明里要写清楚首次打开的步骤。
