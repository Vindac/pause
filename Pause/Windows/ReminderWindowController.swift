import AppKit
import Combine
import SwiftUI

/// 提醒窗口控制器（AppKit）：
/// - 无边框 NSPanel、不抢焦点、透明背景（圆角由 SwiftUI 内容裁切）
/// - 在鼠标所在显示器居中弹出；900×600 基准，小屏自动缩放
/// - 柔和淡入 / 淡出
/// - 默认 floating 层级；勾选"覆盖其他窗口"后升级为 screenSaver 层级
@MainActor
final class ReminderWindowController: ObservableObject {

    static let preferredSize = CGSize(width: 900, height: 600)
    static let cornerRadius: CGFloat = 16

    let panel: NSPanel
    private var cancellables: Set<AnyCancellable> = []
    /// 当前应用的窗口不透明度（0.3–1.0）
    private(set) var appliedOpacity: Double = 1.0

    init(contentView: some View) {
        panel = NSPanel(
            contentRect: NSRect(x: 0, y: 0, width: Self.preferredSize.width, height: Self.preferredSize.height),
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = true
        panel.isMovableByWindowBackground = false
        panel.hidesOnDeactivate = false
        panel.collectionBehavior = [.ignoresCycle, .fullScreenAuxiliary]
        panel.level = .floating

        let hosting = NSHostingView(rootView: contentView)
        // 圆角采用 CALayer 硬裁剪（SwiftUI clipShape 在 NSHostingView 中不可靠）
        hosting.wantsLayer = true
        hosting.layer?.cornerRadius = Self.cornerRadius
        hosting.layer?.cornerCurve = .continuous
        hosting.layer?.masksToBounds = true
        hosting.layer?.isOpaque = false
        hosting.autoresizingMask = [.width, .height]
        hosting.frame = NSRect(origin: .zero, size: Self.preferredSize)
        panel.contentView = hosting
    }

    // MARK: - 显示 / 隐藏

    /// 在指定屏幕居中弹出（按可用区域自适应缩放），淡入到目标不透明度。
    func show(on screen: NSScreen, overlay: Bool, opacity: Double = 1.0) {
        let size = Self.scaledSize(for: screen, preferred: Self.preferredSize)
        panel.setContentSize(size)
        panel.setFrame(Self.centeredFrame(size: size, in: screen), display: false)

        panel.level = overlay ? .screenSaver : .floating
        appliedOpacity = opacity
        panel.alphaValue = 0
        panel.orderFrontRegardless()

        NSAnimationContext.runAnimationGroup { context in
            context.duration = 0.35
            panel.animator().alphaValue = opacity
        }
    }

    /// 窗口已可见则不动（提醒页 → 休息页在同窗口切换）；未显示则弹出。
    func ensureVisible(on screen: NSScreen, overlay: Bool, opacity: Double = 1.0) {
        guard !panel.isVisible else {
            applyOpacity(opacity)
            return
        }
        show(on: screen, overlay: overlay, opacity: opacity)
    }

    /// 运行中调整透明度：已显示的窗口立即平滑应用。
    func applyOpacity(_ opacity: Double) {
        appliedOpacity = opacity
        guard panel.isVisible, panel.alphaValue > 0.02 else { return }
        NSAnimationContext.runAnimationGroup { context in
            context.duration = 0.2
            panel.animator().alphaValue = opacity
        }
    }

    /// 淡出并收起。
    func hide() {
        guard panel.isVisible else { return }
        NSAnimationContext.runAnimationGroup({ context in
            context.duration = 0.5
            panel.animator().alphaValue = 0
        }, completionHandler: { [panel] in
            if panel.alphaValue == 0 {
                panel.orderOut(nil)
            }
        })
    }

    // MARK: - 布局计算（纯函数，可测试）

    /// 小屏自适应：优先 900×600，不够时按可用区域 86% 等比缩放。
    static func scaledSize(for screen: NSScreen, preferred: CGSize) -> CGSize {
        let available = screen.visibleFrame.insetBy(dx: 40, dy: 40)
        let scale = min(
            1,
            available.width / preferred.width,
            available.height / preferred.height
        )
        return CGSize(width: (preferred.width * scale).rounded(.down),
                      height: (preferred.height * scale).rounded(.down))
    }

    static func centeredFrame(size: CGSize, in screen: NSScreen) -> NSRect {
        let origin = NSPoint(
            x: screen.frame.midX - size.width / 2,
            y: screen.frame.midY - size.height / 2
        )
        return NSRect(origin: origin, size: size)
    }

    /// 鼠标所在的屏幕（多显示器策略：跟随用户注意力），兜底主屏 / 第一块屏。
    nonisolated static func screenForPresentation() -> NSScreen {
        let mouse = NSEvent.mouseLocation
        if let hit = NSScreen.screens.first(where: { $0.frame.contains(mouse) }) {
            return hit
        }
        if let main = NSScreen.main {
            return main
        }
        return NSScreen.screens.first!
    }
}
