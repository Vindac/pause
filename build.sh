#!/bin/bash
# 构建 Pause.app（Release, Apple Silicon, ad-hoc 签名）
# 用法: ./build.sh
set -euo pipefail
cd "$(dirname "$0")"

CONFIGURATION="${CONFIGURATION:-release}"
swift build -c "$CONFIGURATION"

BIN_PATH="$(swift build -c "$CONFIGURATION" --show-bin-path)"
APP="build/Pause.app"

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

cp "$BIN_PATH/Pause" "$APP/Contents/MacOS/Pause"
cp Info.plist "$APP/Contents/Info.plist"

# 图标（不存在则生成一次）
if [ ! -f "Resources/Pause.icns" ]; then
    echo ">> Generating app icon…"
    mkdir -p Resources
    swift Scripts/gen_icon.swift "build/Pause.iconset"
    iconutil -c icns -o "Resources/Pause.icns" "build/Pause.iconset"
    rm -rf "build/Pause.iconset"
fi
cp "Resources/Pause.icns" "$APP/Contents/Resources/Pause.icns"

# Ad-hoc 签名，本机可直接运行
codesign --force --sign - "$APP"

echo ""
echo "✅ 构建完成: $PWD/$APP"
echo "   运行: open $APP"
