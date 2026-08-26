import AppKit
import Combine
import SwiftUI

/// 设置窗口控制器（AppKit NSWindow + NSHostingView 装载 SwiftUI 内容）。
/// 不使用 SwiftUI Window scene：在 MenuBarExtra(accessory) 应用中其事件层
/// 存在控件点击不响应的问题；原生窗口的激活/键序可控且已验证可靠。
@MainActor
final class SettingsWindowController {

    private(set) var window: NSWindow?
    private let viewModel: SettingsViewModel
    private let localization: LocalizationStore
    private var cancellables: Set<AnyCancellable> = []

    init(viewModel: SettingsViewModel, localization: LocalizationStore) {
        self.viewModel = viewModel
        self.localization = localization

        // 语言切换时同步窗口标题
        localization.$language
            .receive(on: RunLoop.main)
            .sink { [weak self] _ in
                self?.window?.title = self?.localization.t(.settingsTitle) ?? ""
            }
            .store(in: &cancellables)
    }

    func open() {
        if window == nil {
            let content = SettingsView()
                .environmentObject(viewModel)
                .environmentObject(localization)
            let w = NSWindow(
                contentRect: NSRect(x: 0, y: 0, width: 560, height: 640),
                styleMask: [.titled, .closable, .miniaturizable],
                backing: .buffered, defer: false
            )
            w.title = localization.t(.settingsTitle)
            w.contentView = NSHostingView(rootView: content)
            w.center()
            w.isReleasedWhenClosed = false
            w.setFrameAutosaveName("PauseSettingsWindow")
            window = w
        }
        NSApp.activate(ignoringOtherApps: true)
        window?.makeKeyAndOrderFront(nil)
    }
}
