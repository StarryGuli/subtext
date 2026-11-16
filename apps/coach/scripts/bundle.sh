#!/bin/sh
# 打包 Subtext.app（菜单栏应用，没有 Dock 图标）。产物：target/app/Subtext.app
# 签名默认 ad-hoc：首次打开要在「系统设置 → 隐私与安全性」里放行。有 Developer ID 证书时设 SUBTEXT_SIGN_IDENTITY。
set -e
cd "$(dirname "$0")/../../.."
cargo build --release --locked -p subtext-app
VERSION=$(cargo metadata --offline --no-deps --format-version 1 | python3 -c 'import json,sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"]=="subtext-app"))')
APP=target/app/Subtext.app
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp target/release/subtext "$APP/Contents/MacOS/subtext"
cp apps/coach/Info.plist "$APP/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $VERSION" "$APP/Contents/Info.plist"

ICONSET=$(mktemp -d)/Subtext.iconset
mkdir -p "$ICONSET"
for size in 16 32 128 256 512; do
  sips -z $size $size assets/icon/logo.png --out "$ICONSET/icon_${size}x${size}.png" >/dev/null
  sips -z $((size * 2)) $((size * 2)) assets/icon/logo.png --out "$ICONSET/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/Subtext.icns"

# 仓库在 iCloud 同步目录时新建的 .app 会带 Finder 扩展属性，codesign 会拒：签名前清掉
xattr -cr "$APP"
if [ -n "$SUBTEXT_SIGN_IDENTITY" ]; then
  codesign --force --deep --options runtime --timestamp --sign "$SUBTEXT_SIGN_IDENTITY" "$APP"
else
  codesign --force --deep --sign - "$APP"
fi
echo "已生成 $APP（版本 $VERSION）"
