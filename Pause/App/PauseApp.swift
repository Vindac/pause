import SwiftUI

@main
struct PauseApp: App {

    @NSApplicationDelegateAdaptor(AppDelegate.self) private var appDelegate

    /// 进程唯一的依赖容器（首次访问时惰性构建）
    static let container = DependencyContainer()

    var body: some Scene {
        MenuBarExtra {
            MenuBarView()
                .environmentObject(Self.container.menuBarViewModel)
                .environmentObject(Self.container.localization)
        } label: {
            MenuBarLabelView()
                .environmentObject(Self.container.menuBarViewModel)
        }
        .menuBarExtraStyle(.menu)
        // 设置窗口由 SettingsWindowController(AppKit) 管理：
        // SwiftUI Window scene 在 MenuBarExtra 应用中存在控件点击无响应的问题
    }
}

/// 菜单栏标签：☕ / ☕ 5m / ☕ ⏸
struct MenuBarLabelView: View {
    @EnvironmentObject private var vm: MenuBarViewModel

    var body: some View {
        Text(vm.barLabel)
    }
}

final class AppDelegate: NSObject, NSApplicationDelegate {

    func applicationDidFinishLaunching(_ notification: Notification) {
        // 双保险：裸二进制（swift run）读不到 Info.plist 时也保持菜单栏形态
        NSApp.setActivationPolicy(.accessory)
        PauseApp.container.start()

        if ProcessInfo.processInfo.environment["PAUSE_DEMO"] == "1" {
            runDemoSequence()
        }
        if ProcessInfo.processInfo.environment["PAUSE_DEMO_SETTINGS"] == "1" {
            runDemoSettingsSequence()
        }
    }

    /// 设置窗口演示（PAUSE_DEMO_SETTINGS=1）：通过正式的 SettingsWindowController
    /// 打开设置窗口并截图，验证设置页渲染链路。
    @MainActor
    private func runDemoSettingsSequence() {
        let container = PauseApp.container
        NSApp.activate(ignoringOtherApps: true)
        container.settingsWindowController.open()

        DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) {
            if let window = container.settingsWindowController.window {
                Self.captureWindow(of: window, to: "/tmp/pause_settings_idle.png")
            }
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 2.5) {
            NSApp.terminate(nil)
        }
    }

    /// 演示模式（PAUSE_DEMO=1）：自动 弹提醒 → 截图 → 开始休息 → 截图 → 结束退出，
    /// 用于无人工干预地验证提醒窗口视觉（截自身窗口，无需屏幕录制权限）。
    private func runDemoSequence() {
        let container = PauseApp.container
        let reminder = container.reminder

        DispatchQueue.main.asyncAfter(deadline: .now() + 2.0) {
            reminder.demoTrigger()
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 4.5) {
            Self.captureWindow(of: container.windowController.panel,
                               to: "/tmp/pause_demo_reminder.png")
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 6.0) {
            reminder.startBreak()
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 8.0) {
            Self.captureWindow(of: container.windowController.panel,
                               to: "/tmp/pause_demo_break.png")
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 9.5) {
            NSApp.terminate(nil)
        }
    }

    private static func captureWindow(of window: NSWindow, to path: String) {
        // 用 layer 离屏渲染窗口内容（CGWindowListCreateImage 在 macOS 15+ 已废弃，
        // 且在 macOS 26 上无法完整捕获 NSHostingView 的文字层）
        guard let layer = window.contentView?.layer else { print("[demo] no layer"); return }
        let w = Int(layer.bounds.width), h = Int(layer.bounds.height)
        guard let ctx = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8,
                                  bytesPerRow: 0, space: CGColorSpace(name: CGColorSpace.sRGB)!,
                                  bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else {
            print("[demo] capture failed")
            return
        }
        layer.render(in: ctx)
        guard let rendered = ctx.makeImage() else { print("[demo] render failed"); return }
        let url = URL(fileURLWithPath: path) as CFURL
        if let dest = CGImageDestinationCreateWithURL(url, "public.png" as CFString, 1, nil) {
            CGImageDestinationAddImage(dest, rendered, nil)
            CGImageDestinationFinalize(dest)
            print("[demo] capture → \(path) \(w)x\(h)")
        }
    }
}
