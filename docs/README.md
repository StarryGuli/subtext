# docs

README.md 只介绍项目与怎么用，技术内容放在这里。

| 文件 | 内容 |
|---|---|
| [usage.md](usage.md) | 用户使用说明：教练的触发、后端、配置项、排错 |
| [contributing.md](contributing.md) | 开发约定：架构约束、代码组织、命名与注释、版本号、提交信息、提交前检查 |
| [design/coach.md](design/coach.md) | 双语教练的设计：三种模式、触发、闸门、后端抽象、面板 |
| [design/coach-mockups.html](design/coach-mockups.html) | 面板形态的早期草图（三版），保留作设计记录 |
| [design/architecture.md](design/architecture.md) | Core 与平台层的划分、crate 结构、必须遵守的架构约束、`.qj` 数据容器 |
| [design/candidate-ui.md](design/candidate-ui.md) | 候选窗口、按键约定与翻译 annotation 的设计 |
| [design/rendering.md](design/rendering.md) | 自绘渲染器：显示面与控件面、主题 |
| [design/aux-code.md](design/aux-code.md) | 辅码：触发键与过滤语义、码表导入、设置界面 |
| [design/update.md](design/update.md) | 检查更新：读 GitHub Releases，只提示不安装 |
| [design/landscape.md](design/landscape.md) | 同类项目、可用数据源及其许可 |
| [demo/](demo/) | 教练的示例输出（README 截图用），可用 `subtext-macos --coach-preview` 渲染 |

约定：文档写中文，代码标识符一律英文。实现与文档产生分歧时以代码为准，并同步更新文档。
