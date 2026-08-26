# 休一下（Pause）

[English](README.en.md) | 简体中文

一个 macOS 原生菜单栏休息提醒工具。平时几乎感觉不到它的存在，到点后用一张漂亮的大尺寸自然图片提醒你离开屏幕、看看远处、活动身体。

依据《macOS 休息提醒软件 AI 实现设计方案（MVVM 版）》完整实现：**Swift + SwiftUI + 少量 AppKit，MVVM 架构，零第三方依赖**。最低系统 macOS 13，Apple Silicon 原生构建。当前版本 **v1.0.0**。

## 运行方式

```bash
# 方式一：开发运行（菜单栏应用，注意裸二进制的设置域与 .app 不同）
swift run

# 方式二：构建正式 .app（Release + ad-hoc 签名 + 图标）
./build.sh
open build/Pause.app
```

- 启动后菜单栏直接显示**距下次休息的剩余时间**（如 `43m`，分钟级刷新），应用不占据 Dock（LSUIElement）。
- 休息中菜单栏显示休息剩余分钟；暂停时显示 `⏸`；提醒待弹出时显示 `!`。
- 点菜单：**下次休息倒计时 / 立即休息 / 暂停提醒 / 设置… / 退出**。

## 项目目录说明

```
Pause/
├── App/
│   ├── PauseApp.swift              # @main；MenuBarExtra + 设置窗口；AppDelegate 启动接线
│   ├── AppState.swift              # 跨窗口共享状态的轻量门面（SwiftUI environment 注入）
│   ├── Localization.swift          # 语言状态（中/英，默认英文）+ 全部界面文案内建表
│   └── DependencyContainer.swift   # 组装全部 Service/ViewModel/窗口，phase→窗口编排
├── Models/
│   ├── ReminderSettings.swift      # 全部设置 + 合法范围（10–180 分钟等）+ 默认图片地址
│   ├── ReminderPhase.swift         # 状态机枚举：working/snoozing/reminding/breaking/paused
│   ├── BreakSession.swift          # 休息会话（起止、剩余、mm:ss 格式化）
│   └── WallpaperItem.swift         # 壁纸条目（缓存来源 / 兜底主题）
├── ViewModels/
│   ├── ReminderViewModel.swift     # 提醒页状态（壁纸、可延迟次数、按钮文案·本地化）
│   ├── BreakViewModel.swift        # 休息页会话 + 休息期间壁纸轮换 + 提前结束
│   ├── SettingsViewModel.swift     # 设置绑定，修改即保存；同步 SMAppService；壁纸预览/切换
│   └── MenuBarViewModel.swift      # 菜单栏标签与状态文案（随语言即时重算）
├── Views/
│   ├── WallpaperBackdropView.swift # 壁纸背景组件（Ken Burns + 交叉淡化）+ 玻璃按钮样式
│   ├── ReminderView.swift          # 提醒页（900×600 内：标题/休息时长/延迟休息/开始休息）
│   ├── BreakView.swift             # 休息倒计时页（mm:ss + 提前结束）
│   ├── ReminderContainerView.swift # 窗口内容容器：两页透明度切换（避免淡出时闪空）
│   ├── SettingsView.swift          # 设置窗口（五组：通用/提醒/图片/系统/提醒窗口）
│   └── MenuBarView.swift           # 菜单栏下拉菜单
├── Services/
│   ├── SettingsStore.swift         # UserDefaults 读写 + 范围钳制（键名与文档第 9 节一致）
│   ├── LocalizationStore（见 App/Localization.swift）
│   ├── ReminderService.swift       # 唯一计时真相源：状态机、延迟限制、防提醒风暴
│   ├── WallpaperService.swift      # 壁纸调度：启动装填→后台预取→提醒/手动切换
│   ├── WallpaperProvider.swift     # WallpaperProviding 协议 + 在线下载（地址可自定义）
│   ├── WallpaperCache.swift        # Caches 目录、3200px 降采样、最多 18 张自动淘汰
│   ├── SystemActivityService.swift # 锁屏/屏保/屏幕睡眠监听 + 键鼠空闲检测
│   └── LaunchAtLoginService.swift  # SMAppService 开机启动
├── Windows/
    └── ReminderWindowController.swift # NSPanel：无边框、不抢焦点、多屏定位、淡入淡出
Tests/PauseTests/                    # 30 个单元测试（状态机/空闲顺延/自动休息/缓存淘汰/布局缩放/模型/URL 校验）
Scripts/gen_icon.swift               # 应用图标生成（窗户+远方太阳/山意象）
build.sh                             # Release .app 组装脚本
Info.plist                           # LSUIElement 等应用配置
```

## 关键实现说明

**MVVM 与数据流**：单向数据流 `View → ViewModel → Service → Model/系统`。View 只观察 ViewModel 状态并发送用户意图，不接触 UserDefaults/URLSession/FileManager；ViewModel 把操作转为 Service 调用；Service 封装计时、网络、缓存与系统能力，不依赖任何 View。跨窗口共享的唯一计时真相源是 `ReminderService`（避免多个 Timer 各自运行）。

**状态机**：`working → reminding → breaking → working`；`reminding → snoozing → reminding`（延迟只推迟所设分钟数而非重算完整间隔，次数不限、按钮常驻）；任意时刻可 `paused`。所有转移集中在 `handleTick(_:)` 与用户动作方法中，注入时钟与系统状态协议后完全可测试。

**自动开始休息**：默认开启。提醒弹出后「开始休息」按钮文案附带逐秒倒计时（如「开始休息（30 秒）」，默认 30 秒，可设 10/20/30/60 秒），期间无操作自动进入休息；期间仍可「延迟休息」（延迟后再提醒时重新倒计时）或「开始休息」。设置中可关闭，回到纯手动模式。

**按真实使用时间计时**：默认开启。通过 `CGEventSource` 读取距上次键鼠输入的秒数（只读时间戳，无需辅助功能权限）；无输入达到「离开判定」时长（默认 2 分钟）后，工作/延迟倒计时自动顺延——离开电脑、显示器睡眠、系统睡眠的时间都不计入。因此唤醒/回来后不会立即弹提醒，而是**累计真实使用满间隔才提醒**。设置中可关闭（回到固定时间模式）或调整离开判定（1/2/3/5 分钟）；离开期间菜单栏显示「未检测到使用 · 计时已暂停」。

**多语言**：界面支持 中文 / English 切换（默认英文）。文案以 `L10nKey` 内建中英表维护（SPM 可执行目标无 bundle 本地化资源，内建表才能支持应用内运行时切换）；`LocalizationStore` 持久化选择并在切换瞬间驱动菜单栏、提醒页、休息页、设置页全部重算。

**防提醒风暴**：deadline 为绝对时间；系统睡眠期间 Timer 冻结，唤醒后第一次 tick 只会把"一个已过期 deadline"转换为一次提醒。仅锁屏 / 屏保 / 屏幕睡眠（弹了也看不见）时到期提醒暂缓，环境恢复后 1 秒内弹出；用户活跃使用时**到点直接弹出**，不做前台全屏检测（几何判断会把最大化大窗口误判为全屏，导致提醒一直不弹）。

**壁纸管线**：启动即从缓存装填当前图并后台预取下一张；提醒到达时**只使用已就绪的本地图片**，从不等待网络。默认从 picsum.photos 随机取图（也可以通过 `defaults write` 配置 `wallpaperImageURLString` 为任意 http(s) 图片地址）。降级链：预取图 → 较旧缓存 → CoreGraphics 运行时生成的渐变风景（首次运行、无网络均可用，仅作最终兜底、不作为独立来源）。下载图片统一降采样到 ≤3200px 再落盘，缓存最多 18 张、自动淘汰最旧。设置中提供「切换图片」按钮与当前壁纸预览。图片服务抽象为 `WallpaperProviding`，未来切换 Unsplash/Pexels 只需替换 Provider。

**多显示器与窗口形态**：提醒窗口在鼠标所在显示器居中弹出；900×600 基准按可用区域自动等比缩放（保持 3:2）。窗口为无边框 NSPanel（nonactivating，不抢焦点），内容层通过 CALayer 以 16pt 连续圆角硬裁剪；默认 floating 层级，勾选"覆盖其他窗口"后升级；柔和淡入 0.35s / 淡出 0.5s。设置中的"窗口透明度"（30%–100%）实时控制提醒窗口不透明度，显示中修改立即生效。

**演示模式（隐藏）**：`PAUSE_DEMO=1` 直接运行二进制会自动演示完整流程（弹提醒 → 开始休息 → 自动退出），用于自动化视觉验证。

**动效**：背景图为 aspectFill + 极慢 Ken Burns 缩放（1.00→1.05，45s 往返），切图 0.8s 交叉淡化；休息期间每 25 秒换一张。菜单栏倒计时为分钟级发布，空闲时几乎无 SwiftUI 刷新。

**开机启动**：SMAppService（macOS 13+）。在 .app 内运行才会注册成功；`swift run` 裸二进制下仅记录失败，不影响其他功能。

## 已知限制

- 在线图片源不支持主题过滤；图片完全来自网络（默认 picsum.photos 随机图）。
- 首次运行且无网络时使用运行时生成的渐变风景图（仅最终兜底，非独立图片来源）。
- 其他 App 全屏演示/看片时提醒会照常弹出（窗口不抢焦点、默认浮动层级）；锁屏/屏保/显示器睡眠时暂缓，恢复后自动弹出。
- `swift run` 与 `build/Pause.app` 的 UserDefaults 域不同，两者的设置互不共享；正式使用请以 .app 方式运行。
- Ad-hoc 签名仅本机可运行；分发需开发者证书重新签名。

## 测试

```bash
swift test    # 30 个用例：状态机全流程 / 无限延迟 / 空闲顺延 / 自动休息 / 唤醒防风暴 / 缓存淘汰 / 降采样 / 小屏缩放 / URL 校验
```

## Release 构建

```bash
./build.sh
# 产物: build/Pause.app（Release、arm64、零第三方运行时依赖、ad-hoc 签名）
# 分发: codesign --force --sign "Developer ID Application: …" build/Pause.app
```

## 首版功能范围

macOS 菜单栏 App · 中/英文界面切换（默认英文）· 30/45/60/自定义（10–180 分钟）间隔 · 按真实使用时间计时（离开/睡眠暂停，默认开启）· 自定义休息时长 · 延迟休息 1–5 分钟/自定义（次数不限）· 900×600 自适应提醒窗口（16pt 圆角）· 网络壁纸 + 本地缓存 + 运行时兜底 · 手动切换图片 · Ken Burns + 交叉淡化 · 休息倒计时 · 暂停/继续 · 立即休息 · 开机启动 · 设置自动保存 · 浅色/深色适配 · 多显示器 · 锁屏/屏保/睡眠避让。

明确不做：账号、云同步、任务管理、打卡、复杂统计、社区、订阅、广告、插件系统。
