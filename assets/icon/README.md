# 图标

- `logo.png`（866×866，带透明通道）：应用图标源文件。`apps/macos/scripts/bundle.sh` 打包时用 `sips` + `iconutil`
  生成 `Qingjian.icns`，生成物不进仓库。
- `menu.svg`：macOS 输入法菜单 / 菜单栏图标源文件（黑色圆角矩形镂空「青简」，与系统自带输入法同一风格）。
  `menu.pdf` 是它导出的矢量版（高 16pt），打包时原样拷成 `qingjian-menu.pdf`；Info.plist 标了 `TISIconIsTemplate`，
  系统按菜单栏深浅色自动反色，不用备两套。改了 svg 重新导出：

  ```sh
  rsvg-convert -f pdf -h 16pt --keep-aspect-ratio assets/icon/menu.svg -o assets/icon/menu.pdf
  ```
