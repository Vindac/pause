休一下（Pause）v2.0.0 Windows 便携版
=====================================

【使用方法】
  直接双击 pause.exe 运行，程序常驻系统托盘（右下角）。

【系统要求】
  - Windows 10 1809 及以上 / Windows 11（x64）
  - 需要 WebView2 运行时（Win11 已内置；Win10 大多已随 Edge 安装）。
    若启动无反应，请安装：
    https://developer.microsoft.com/microsoft-edge/webview2/

【便携说明】
  - 本版本无需安装、不写注册表；配置存储在
    %APPDATA%\com.pause.Pause\settings.json
    壁纸缓存在 %LOCALAPPDATA%\com.pause.Pause\Pause\Wallpapers\
  - 删除程序 = 删除 pause.exe 与上述两个目录即可完全卸载。

【开机启动】
  设置中开启「开机自动启动」会写入当前用户的启动项（HKCU Run），
  该注册表键随程序删除不会自动清理，介意可先在设置中关闭。

休一下 Pause — 菜单栏休息提醒工具 (macOS & Windows)
MIT License (c) 2026 Vindac
