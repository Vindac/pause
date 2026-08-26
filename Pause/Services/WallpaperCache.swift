import AppKit
import Foundation

/// 壁纸磁盘缓存：存放在系统 Caches 目录的应用专属子目录下。
/// 职责：图片降采样后落盘、按新→旧顺序索引、自动淘汰旧图，避免长期增长。
final class WallpaperCache {

    /// 保留张数上限（文档建议 10–20 张）
    static let maxCount = 18
    /// 存储与显示的最大像素边（Retina 全屏显示足够，同时控制内存）
    static let maxPixelSize: CGFloat = 3200

    let directory: URL
    private let indexURL: URL
    private let ioQueue = DispatchQueue(label: "pause.wallpapercache.io")
    private let queue = DispatchQueue(label: "pause.wallpapercache")

    init(directory: URL? = nil) {
        let base = directory ?? FileManager.default
            .urls(for: .cachesDirectory, in: .userDomainMask).first!
            .appendingPathComponent("Pause/Wallpapers", isDirectory: true)
        self.directory = base
        self.indexURL = base.appendingPathComponent("index.plist")
        try? FileManager.default.createDirectory(at: base, withIntermediateDirectories: true)
    }

    // MARK: - 索引（新 → 旧）

    private func loadOrder() -> [String] {
        (NSArray(contentsOf: indexURL) as? [String]) ?? []
    }

    private func saveOrder(_ order: [String]) {
        (order as NSArray).write(to: indexURL, atomically: true)
    }

    /// 纯函数：给定当前顺序与上限，计算应淘汰的文件名（可测试）
    static func filesToEvict(orderedNames: [String], maxCount: Int) -> [String] {
        guard orderedNames.count > maxCount else { return [] }
        return Array(orderedNames[maxCount...])
    }

    // MARK: - 写入

    /// 将下载数据降采样为 JPEG 后写入缓存，返回落盘 URL；失败返回 nil。
    func add(_ data: Data) -> URL? {
        guard let rep = Self.downsampledJPEG(data: data, maxPixel: Self.maxPixelSize) else { return nil }
        let name = UUID().uuidString + ".jpg"
        let url = directory.appendingPathComponent(name)
        do {
            try rep.write(to: url)
        } catch {
            return nil
        }

        var order = loadOrder()
        order.insert(name, at: 0)
        let evict = Self.filesToEvict(orderedNames: order, maxCount: Self.maxCount)
        if !evict.isEmpty {
            order.removeLast(evict.count)
            evict.forEach { try? FileManager.default.removeItem(at: directory.appendingPathComponent($0)) }
        }
        saveOrder(order)
        return url
    }

    // MARK: - 读取

    /// 最新一张
    func latest() -> URL? {
        loadOrder().first.map { directory.appendingPathComponent($0) }
    }

    /// 随机取一张较旧的（在线获取失败时的回退素材）
    func randomOlder(excludingFirst: Bool) -> URL? {
        let order = loadOrder()
        let candidates = excludingFirst && order.count > 1 ? Array(order.dropFirst()) : order
        guard let name = candidates.randomElement() else { return nil }
        return directory.appendingPathComponent(name)
    }

    var count: Int { loadOrder().count }

    // MARK: - 降采样

    /// 纯工具：将原始图片数据解码 → 缩到 maxPixel 内 → JPEG representation。
    /// 避免把超大原图完整解码进内存并长期保存。
    static func downsampledJPEG(data: Data, maxPixel: CGFloat) -> Data? {
        guard let source = CGImageSourceCreateWithData(
            data as CFData, [kCGImageSourceShouldCache: false] as CFDictionary
        ) else { return nil }
        let options: [CFString: Any] = [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceThumbnailMaxPixelSize: maxPixel,
            kCGImageSourceShouldCacheImmediately: true
        ]
        guard let cgImage = CGImageSourceCreateThumbnailAtIndex(source, 0, options as CFDictionary) else {
            return nil
        }
        let rep = NSBitmapImageRep(cgImage: cgImage)
        return rep.representation(using: .jpeg, properties: [.compressionFactor: 0.82])
    }
}
