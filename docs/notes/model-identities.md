# 含章模型名称与检查点

「含章」是青简本地神经模型系列。下面的检查点编号来自现有模型配置，用于追溯权重；两款模型尚未指定正式发行版本。应用版本和数据 Release 标签也不充当模型版本。

| 模型 | 英文名 | 标识 | 用途 | 当前检查点 |
| --- | --- | --- | --- | --- |
| 含章·通变 | Hanzhang Tongbian — neural pinyin decoding and correction | `hanzhang-tongbian` | 拼音到汉字解码、纠错、整句候选重排与词图未覆盖时的生成 | `small-15000` |
| 含章·知微 | Hanzhang Zhiwei — context-aware candidate rescoring | `hanzhang-zhiwei` | 依据光标前文给整句候选重排；没有通变时作为回退 | `small-155478` |

`tools/release/pack-model.sh` 把正式名称写进 `.qjm` 的 `name` 元数据，`version` 暂记 `<标识>-<检查点>`，例如 `hanzhang-tongbian-small-15000`。这里的 `version` 字段是权重追溯标识，不宣称正式版本号。文件名仍为 `model.qjm`，两类模型分别放在 `models/hanzhang-tongbian/` 与 `models/hanzhang-zhiwei/`；数据 Release 用独立的 `data-vN` 标签和 SHA-256 锁定具体字节。正式版本确定之前不在目录名中加入版本号。

改名称或元数据时，两份模型都要重新打包并重算哈希；不能只改脚本而沿用旧 `.qjm`。正式发布还要将新模型文件放进新的不可变数据 Release，并更新 `tools/release/data.lock`。
