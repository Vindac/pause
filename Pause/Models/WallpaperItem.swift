import AppKit
import Foundation

/// 一张用于提醒窗口背景的壁纸。
/// Identifiable：视图层据此驱动交叉淡化动画。
struct WallpaperItem: Identifiable {
    enum Origin {
        case networkCache(URL)      // 来自在线下载并落盘的缓存
        case builtin(WallpaperTheme) // 内置（运行时生成）
    }

    let id: UUID
    let origin: Origin
    let image: NSImage

    init(origin: Origin, image: NSImage) {
        self.id = UUID()
        self.origin = origin
        self.image = image
    }
}
