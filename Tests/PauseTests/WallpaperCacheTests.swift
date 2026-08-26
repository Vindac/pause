import XCTest
@testable import Pause

/// 缓存淘汰与降采样纯逻辑测试。
final class WallpaperCacheTests: XCTestCase {

    func testEvictPlanPureFunction() {
        let names = (1...25).map { "img\($0).jpg" }   // 新 → 旧
        let evict = WallpaperCache.filesToEvict(orderedNames: names, maxCount: 18)
        XCTAssertEqual(evict, ["img19.jpg", "img20.jpg", "img21.jpg", "img22.jpg", "img23.jpg", "img24.jpg", "img25.jpg"])
        XCTAssertTrue(WallpaperCache.filesToEvict(orderedNames: names, maxCount: 30).isEmpty)
        XCTAssertTrue(WallpaperCache.filesToEvict(orderedNames: [], maxCount: 18).isEmpty)
    }

    func testCacheAddAndEvictOnDisk() throws {
        let tmp = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("cache-test-\(UUID().uuidString)")
        let cache = WallpaperCache(directory: tmp)
        defer { try? FileManager.default.removeItem(at: tmp) }

        let image = try XCTUnwrap(BuiltInWallpaperProvider.render(theme: .minimal))
        let tiff = try XCTUnwrap(image.tiffRepresentation)
        let rep = try XCTUnwrap(NSBitmapImageRep(data: tiff))
        let jpeg = try XCTUnwrap(rep.representation(using: .jpeg, properties: [:]))

        for _ in 0..<(WallpaperCache.maxCount + 5) {
            XCTAssertNotNil(cache.add(jpeg))
        }
        XCTAssertLessThanOrEqual(cache.count, WallpaperCache.maxCount)
        XCTAssertNotNil(cache.latest())
        let files = try FileManager.default.contentsOfDirectory(at: tmp, includingPropertiesForKeys: nil)
            .filter { $0.pathExtension == "jpg" }
        XCTAssertLessThanOrEqual(files.count, WallpaperCache.maxCount)
    }

    func testDownsampleReducesPixelSize() throws {
        // 画一张 4000x4000 的测试图
        let size = 4000
        guard let ctx = CGContext(
            data: nil, width: size, height: size, bitsPerComponent: 8, bytesPerRow: 0,
            space: CGColorSpace(name: CGColorSpace.sRGB)!,
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        ) else { return XCTFail("context") }
        ctx.setFillColor(CGColor(red: 0.3, green: 0.6, blue: 0.5, alpha: 1))
        ctx.fill(CGRect(x: 0, y: 0, width: size, height: size))
        let image = try XCTUnwrap(ctx.makeImage())
        let rep = NSBitmapImageRep(cgImage: image)
        let jpeg = try XCTUnwrap(rep.representation(using: .jpeg, properties: [.compressionFactor: 1.0]))

        let downsampled = try XCTUnwrap(WallpaperCache.downsampledJPEG(data: jpeg, maxPixel: 3200))
        let read = try XCTUnwrap(NSBitmapImageRep(data: downsampled))
        XCTAssertLessThanOrEqual(max(read.pixelsWide, read.pixelsHigh), 3200)
        XCTAssertNil(WallpaperCache.downsampledJPEG(data: Data("junk".utf8), maxPixel: 3200))
    }
}

final class ModelTests: XCTestCase {

    func testCartoonThemeRenders() {
        // 卡通插画三种场景随机生成，均应产出有效图像
        for _ in 0..<6 {
            XCTAssertNotNil(BuiltInWallpaperProvider.render(theme: .cartoon))
        }
        XCTAssertNotNil(BuiltInWallpaperProvider.render(theme: .nature))
        XCTAssertNotNil(BuiltInWallpaperProvider.render(theme: .cityNight))
        XCTAssertNotNil(BuiltInWallpaperProvider.render(theme: .minimal))
    }

    func testBreakSessionRemainingAndFormat() {
        let start = Date(timeIntervalSince1970: 0)
        let session = BreakSession(startedAt: start, duration: 300)
        XCTAssertEqual(session.remaining(at: start), 300, accuracy: 0.001)
        XCTAssertEqual(session.remaining(at: start.addingTimeInterval(299)), 1, accuracy: 1.01)
        XCTAssertEqual(session.remaining(at: start.addingTimeInterval(999)), 0, accuracy: 0.001)

        XCTAssertEqual(BreakSession.format(300), "05:00")
        XCTAssertEqual(BreakSession.format(283), "04:43")
        XCTAssertEqual(BreakSession.format(0), "00:00")
    }

    func testSettingsDefaultsAndClamp() {
        let suite = "test.defaults.\(UUID().uuidString)"
        let d = UserDefaults(suiteName: suite)!
        d.removePersistentDomain(forName: suite)
        defer { d.removePersistentDomain(forName: suite) }

        let settings = SettingsStore.load(from: d)
        XCTAssertEqual(settings.reminderIntervalMinutes, 45)
        XCTAssertEqual(settings.breakDurationMinutes, 5)
        XCTAssertEqual(settings.snoozeMinutes, 5)
        XCTAssertEqual(settings.wallpaperImageURLString, "")
        XCTAssertEqual(settings.wallpaperTheme, .nature)
        XCTAssertTrue(settings.launchAtLogin)
        XCTAssertTrue(settings.soundEnabled)
        XCTAssertFalse(settings.overlayOtherWindows)
        XCTAssertTrue(settings.activityBasedTiming)
        XCTAssertEqual(settings.idleThresholdMinutes, 2)
        XCTAssertTrue(settings.autoStartBreak)
        XCTAssertEqual(settings.autoStartBreakDelaySeconds, 30)
        XCTAssertEqual(settings.reminderWindowOpacity, 1.0)

        // 非法值回退默认
        d.set(5, forKey: SettingsStore.Key.reminderIntervalMinutes)
        d.set(999, forKey: SettingsStore.Key.breakDurationMinutes)
        d.set(0.05, forKey: SettingsStore.Key.reminderWindowOpacity)
        let clamped = SettingsStore.load(from: d)
        XCTAssertEqual(clamped.reminderIntervalMinutes, 45)
        XCTAssertEqual(clamped.breakDurationMinutes, 5)
        XCTAssertEqual(clamped.reminderWindowOpacity, 1.0)   // 低于下限回退默认
    }

    // MARK: - 图片地址校验

    func testWallpaperURLValidation() {
        XCTAssertTrue(SettingsStore.isHTTPURL("https://picsum.photos/2880/1800"))
        XCTAssertTrue(SettingsStore.isHTTPURL("http://example.com/a.jpg"))
        XCTAssertFalse(SettingsStore.isHTTPURL("ftp://example.com/a.jpg"))
        XCTAssertFalse(SettingsStore.isHTTPURL("example.com/a.jpg"))
        XCTAssertFalse(SettingsStore.isHTTPURL(""))
        XCTAssertFalse(SettingsStore.isHTTPURL("not a url"))

        // 地址默认留空（= 默认图片服务）；非法存储值归一化为空；合法自定义地址保留
        let suite = "test.url.\(UUID().uuidString)"
        let d = UserDefaults(suiteName: suite)!
        d.removePersistentDomain(forName: suite)
        defer { d.removePersistentDomain(forName: suite) }

        XCTAssertEqual(SettingsStore.load(from: d).wallpaperImageURLString, "")

        d.set("bad url", forKey: SettingsStore.Key.wallpaperImageURLString)
        XCTAssertEqual(SettingsStore.load(from: d).wallpaperImageURLString, "")

        d.set("https://images.example.com/photo.jpg",
              forKey: SettingsStore.Key.wallpaperImageURLString)
        XCTAssertEqual(SettingsStore.load(from: d).wallpaperImageURLString,
                       "https://images.example.com/photo.jpg")
    }
}

@MainActor
final class WindowLayoutTests: XCTestCase {

    func testScaledSizeOnLargeScreenKeepsPreferred() {
        let screen = FakeScreen(frame: NSRect(x: 0, y: 0, width: 3000, height: 2000),
                                visible: NSRect(x: 0, y: 0, width: 3000, height: 1900))
        let size = ReminderWindowController.scaledSize(
            for: screen, preferred: ReminderWindowController.preferredSize)
        XCTAssertEqual(size.width, 900, accuracy: 0.5)
        XCTAssertEqual(size.height, 600, accuracy: 0.5)
    }

    func testScaledSizeOnSmallScreenShrinksProportionally() {
        // 小屏（如 1280×720）下按可用区域等比缩小
        let screen = FakeScreen(frame: NSRect(x: 0, y: 0, width: 1280, height: 720),
                                visible: NSRect(x: 0, y: 40, width: 1280, height: 640))
        let size = ReminderWindowController.scaledSize(
            for: screen, preferred: ReminderWindowController.preferredSize)
        XCTAssertLessThan(size.width, 900)
        XCTAssertLessThan(size.height, 600)
        // 保持 3:2 比例
        XCTAssertEqual(size.width / size.height, 1.5, accuracy: 0.02)
    }
}

/// 测试用的 NSScreen 替身：scaledSize 只依赖 visibleFrame。
final class FakeScreen: NSScreen {
    private let frameRect: NSRect
    private let visibleRect: NSRect
    init(frame: NSRect, visible: NSRect) {
        self.frameRect = frame
        self.visibleRect = visible
        super.init()
    }
    override var frame: NSRect { frameRect }
    override var visibleFrame: NSRect { visibleRect }
}
