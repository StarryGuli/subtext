# 参与开发

Subtext 是个人维护的项目，欢迎 Issue 与 PR。动手前请先读 [docs/contributing.md](docs/contributing.md)：架构上不能越的线、代码怎么组织、提交信息怎么写。

## 先开 Issue 的情况

- 想加新功能或改变现有行为：先开 Issue 说清楚想解决什么问题。
- 教练的提示词（`crates/subtext-coach/src/prompt/`）直接决定输出质量，改动请附上改前改后的真实输出对比。

## 本地开发

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
```

构建安装包需要产品数据，见 README「From source」一节。输入法壳（IMK）不能 `cargo run`，需要打包后安装；
面板外观可以不装输入法直接预览：

```bash
subtext-macos --coach-preview decode docs/demo/decode.json out.png dark
```

## PR 清单

- [ ] 一个 PR 只做一件事
- [ ] `cargo test --workspace` 与 clippy 通过
- [ ] 改了用户能感知的行为：同步 [docs/usage.md](docs/usage.md)
- [ ] 改了隐私相关的逻辑（闸门、外发内容）：补测试，并在 PR 里说明
- [ ] 改壳里行为（按键、上屏、面板位置）的修复要真机验证，写明系统版本、应用与步骤
