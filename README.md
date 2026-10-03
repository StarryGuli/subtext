<h1 align="center">Subtext 言外</h1>

<p align="center"><strong>双语输入法：读得懂潜台词，写得出地道话。</strong></p>

Subtext 是一款带 AI 教练的中英双语输入法，目前开发中（macOS 优先）。

- **复制英文 → 自动解码。** 复制一段别人发来的英文，面板给出情境、俚语与缩写、语气和潜台词，译文默认折叠，让你先自己猜。
- **输入中文 → 给出英文表达。** 打完一句中文，按语境给出可直接发送的英文，并说明为什么这样说。语气（随意 / 中性 / 正式）和是否缩写由上下文决定。
- **后端自选。** 本机 Claude Code CLI、本机 Codex CLI、自填 OpenAI 兼容 API、Anthropic API，设置里切换。

设计与行为约定见 [docs/design/coach.md](docs/design/coach.md)，面板草图见 [docs/design/coach-mockups.html](docs/design/coach-mockups.html)。

## 致谢与许可

Subtext 的输入法内核基于开源项目 [青简 Qingjian](https://github.com/qingjian-team/qingjian) 二次开发，在此致谢。这是独立的衍生项目，不并回上游。

代码沿用 [GPL-3.0-or-later](LICENSE)。青简的项目名称与 logo 不包含在代码授权内，本项目不使用。随包数据有各自的来源与许可，见 [数据来源清单](docs/design/landscape.md)。
