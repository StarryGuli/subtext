## 改了什么

<!-- 一两句：解决什么问题、怎么解的。关联的 Issue 写「关闭 #编号」。 -->

## 合并前清单

- [ ] `cargo fmt --all --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace` 本机全过
- [ ] 用户能感知的行为变了（按键、面板、设置项、配置文件），[docs/usage.md](../docs/usage.md) 已同步
- [ ] 改了外发闸门或隐私相关逻辑：补了测试，并在下面说明
- [ ] 改了教练提示词：附上改前改后的真实输出对比
- [ ] 提交信息用 Conventional Commits（`fix(coach): ……`，说明写中文）；不改 `CHANGELOG.md`

## 怎么验证的

<!-- 修 bug、改按键 / 上屏 / 面板位置的 PR 必填：在自己机器上复现过、改完在同一个应用里确认修好，
     写明系统版本、应用与操作步骤。编译与 CI 通过不算验证。 -->
