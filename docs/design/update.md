# 检查更新

输入法不替换自己：只提示，点了打开 GitHub Releases 的最新版页面，安装仍由系统安装程序完成。

## 流程

`crates/subtext-update`：激活期间每天一次，起一个一次性线程请求
`https://api.github.com/repos/StarryGuli/subtext/releases`，把每个 Release 转成内部的 `Release`：

- 版本号取 `tag_name` 去掉前缀 `v`；草稿不算。
- 渠道：版本号里带 `alpha` / `beta` / `rc` 就归对应渠道；GitHub 标了预发布但版本号没写的归 `beta`；其余是 `stable`。
- 安装包：文件名以 `.pkg` 结尾，按名字里的 `arm64` / `x86_64` 认 CPU，`universal` 两种都算。
- 更新日志：Release 正文的非空、非标题行，最多 8 行。

再按 偏好设置 里选的渠道（正式版 / 测试版）与这台机器的 CPU，挑出比当前新的最新一个，结果写进数据目录的 `update.json`，
菜单与「关于」页读它显示。请求只带 `subtext/<版本>` 的 User-Agent，没有任何标识。

## 为什么不验签

青简的版本索引由发版方私钥签名、客户端内置公钥验签，因为它走自己的官网。Subtext 直接读 GitHub 的 HTTPS 接口，
更新**只是提示**、不下载不执行，所以不再自建签名体系。安装包的完整性靠 Release 里的 `SHA256SUMS` 由用户核对。

私有仓库的接口对匿名请求返回 404，客户端当作「没有新版」，不报错。
