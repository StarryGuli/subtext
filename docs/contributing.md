# 开发约定

改代码前先看这一页：架构上不能越的线、代码怎么组织、版本号与提交信息怎么写。
设计的来龙去脉在 [design/architecture.md](design/architecture.md) 与 [design/coach.md](design/coach.md)。

## 架构约束（不要违反）

- **`subtext-coach` 平台无关。** 闸门、提示词、后端、后台服务、输出解析都在这里，不依赖 AppKit。应用层（`apps/coach`）只做两件事：
  把用户的操作变成请求，把结果画出来。**应用层不允许出现提示词、网络请求。**
- **界面不阻塞。** 请求在后台线程里发，主线程只往通道里丢请求、取事件。
- **外发必须过闸门。** 凡是会把用户的文字发往后端的路径，一律先过 `subtext_coach::gate::Gate`；新增触发源要补闸门测试。
  闸门宁可多拦：漏拦的代价远大于误拦。
- **只在用户亲手提交时发请求。** 不做自动读取剪贴板或屏幕后发送。
- **显示面自绘、控件面原生。** 结果区用 `Doc` → `NSAttributedString` 排版，控件用 AppKit 原生。

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
- 密钥只进数据目录的 `.env`，不进 `config.toml`，不进日志。

## 版本号

- `crates/*` 用 `version.workspace = true`；`apps/coach` 是独立发布的产品，写死自己的 `version`。
- 发版之间带 `-dev`，打包脚本再接 git 短哈希；发版提交去掉 `-dev` 打标签 `v<版本>`，标签后再改成下一个 `-dev`。
- 改了 `apps/coach/Cargo.toml` 的版本后用 `cargo metadata --offline` 同步 `Cargo.lock`，否则 `bundle.sh` 的 `--locked` 会报错。

## 提交信息

[Conventional Commits](https://www.conventionalcommits.org/zh-hans/)：第一行 `<类型>(<范围>): <说明>`，类型与范围英文小写，说明用中文。

- 类型：`feat` / `fix` / `docs` / `refactor` / `perf` / `test` / `build` / `ci` / `chore` / `style` / `revert`。
- 范围：crate 或应用的名字——`coach` `app` `tools` `docs` `ci` `deps` `brand` `release`。
- 正文写「为什么」与取舍，一行一条；不加 AI 署名。

## 提交前检查

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

`.githooks/` 里有对应的钩子，`git config core.hooksPath .githooks` 启用一次。
教练提示词的改动用 `subtext-coach --print-prompt` 看实际发出去的内容，用真实模型的回复文件配 `--from-reply` 验证解析；界面改动用 `subtext --snapshot <模式> <回复.json> <输入文字> <输出.png>` 出图看（不要屏幕录制权限）。

## 发版

1. 改 `apps/coach/Cargo.toml` 去掉 `-dev`，同步 `Cargo.lock`，更新 `CHANGELOG.md`。
2. `apps/coach/scripts/bundle.sh` 出 `target/app/Subtext.app`，压成 zip。
3. 打标签 `v<版本>`，在 GitHub 上建 Release，附上 zip 与 `SHA256SUMS`。有 Developer ID 证书时设 `SUBTEXT_SIGN_IDENTITY` 再打包；
   没有就是 ad-hoc 签名，Release 说明里要写清楚首次打开的步骤。
