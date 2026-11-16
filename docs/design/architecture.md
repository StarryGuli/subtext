# 架构

言外 0.4 起是一个只做英文教练的菜单栏应用。0.3 及之前的拼音输入法（Input Method Kit 壳 + 引擎 + 词库 + 自绘候选窗）已移除，
代码保留在 `v0.3.6` 标签与 `main` 的历史里。

```text
crates/subtext-coach   平台无关：闸门、提示词、后端（4 个）、后台服务与缓存、输出解析；自带命令行
apps/coach             macOS 应用：窗口、菜单栏图标、全局热键、结果排版（Doc → NSAttributedString）
```

## 约束

- `subtext-coach` 不依赖任何平台 API，不依赖 AppKit。应用层只做两件事：把用户的操作变成请求，把结果画出来。
- 外发必须过闸门：凡是把用户文字发往后端的路径，先过 `Gate`（手动提交走 `check_manual`）。
- 界面线程不阻塞：请求在后台线程里发，界面用 0.1 秒定时器取事件，「最新请求优先」，过期结果丢弃。
- 模式由用户手动选，教练不猜；窗口只在用户按下「解析」时才发请求。

## 数据流

```text
输入框文字 + 模式 ──▶ Gate::check_manual ──▶ CoachService.submit ──▶ 后台线程
                                                                      │ 提示词 + 后端
结果区 ◀── Doc ◀── CoachOutput ◀── Partial / Finished / Failed ◀──────┘
```

## 未接入的部分

`subtext-coach/src/screen/`（屏幕阅读的平台无关部分）和 `tools/ocr/` 是输入法时代的屏幕阅读，目前没有应用使用它们。
