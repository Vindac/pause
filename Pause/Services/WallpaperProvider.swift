import AppKit
import CoreGraphics
import Foundation

// MARK: - Provider 协议

/// 壁纸来源抽象。UI 与调度逻辑不依赖任何具体图片服务。
/// 协议方法在后台线程执行，返回 nil 表示本次获取失败（调用方降级）。
/// url：设置中配置的在线取图地址；Provider 可忽略（如兜底渲染）。
protocol WallpaperProviding {
    func fetchNext(url: URL?) async -> NSImage?
}

// MARK: - 在线 Provider（默认 picsum.photos）

/// 从设置中配置的任意 http(s) 图片地址下载壁纸（默认 picsum.photos：
/// 免费、稳定、无需注册与密钥，每次请求随机返回不同图片）。
final class OnlineWallpaperProvider: WallpaperProviding {

    private let cache: WallpaperCache
    private let session: URLSession
    private let defaultURL: URL

    init(cache: WallpaperCache,
         defaultURLString: String = ReminderSettings.defaultWallpaperImageURLString,
         timeout: TimeInterval = 15) {
        self.cache = cache
        self.defaultURL = URL(string: defaultURLString) ?? URL(string: "https://picsum.photos/2880/1800")!
        let config = URLSessionConfiguration.ephemeral
        config.timeoutIntervalForRequest = timeout
        config.timeoutIntervalForResource = timeout * 2
        config.requestCachePolicy = .reloadIgnoringLocalCacheData   // 同一地址也能取到新图
        self.session = URLSession(configuration: config)
    }

    func fetchNext(url: URL?) async -> NSImage? {
        let target = url ?? defaultURL
        for _ in 0..<2 {
            if let data = await download(from: target), let saved = cache.add(data) {
                return NSImage(contentsOfFile: saved.path)
            }
        }
        return nil
    }

    private func download(from url: URL) async -> Data? {
        do {
            var request = URLRequest(url: url)
            request.cachePolicy = .reloadIgnoringLocalCacheData
            let (data, response) = try await session.data(for: request)
            guard let http = response as? HTTPURLResponse, http.statusCode == 200,
                  data.count > 30_000 else { return nil }
            return data
        } catch {
            return nil
        }
    }
}

// MARK: - 兜底 Provider（CoreGraphics 运行时生成）

/// 零资源文件的最终兜底：首次运行且无缓存、或离线时使用，
/// 保证提醒窗口永远有背景。生成柔和的渐变自然风景。
final class BuiltInWallpaperProvider: WallpaperProviding {

    func fetchNext(url: URL?) async -> NSImage? {
        nil   // 仅作为同步渲染的兜底实现，不参与在线预取
    }

    static func render(theme: WallpaperTheme) -> NSImage? {
        let size = CGSize(width: 1600, height: 1000)
        guard let cg = CGContext(
            data: nil, width: Int(size.width), height: Int(size.height),
            bitsPerComponent: 8, bytesPerRow: 0,
            space: CGColorSpace(name: CGColorSpace.sRGB)!,
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        ) else { return nil }

        let w = size.width, h = size.height

        func gradient(_ colors: [CGColor], from: CGPoint, to: CGPoint) {
            cg.saveGState()
            if let g = CGGradient(colorsSpace: nil, colors: colors as CFArray, locations: nil) {
                cg.drawLinearGradient(g, start: from, end: to, options: [])
            }
            cg.restoreGState()
        }

        switch theme {
        case .nature:
            // 天空：暖光黄昏 → 上方深青
            gradient([CGColor(red: 0.98, green: 0.86, blue: 0.70, alpha: 1),
                      CGColor(red: 0.45, green: 0.62, blue: 0.62, alpha: 1),
                      CGColor(red: 0.16, green: 0.28, blue: 0.33, alpha: 1)],
                     from: CGPoint(x: 0, y: h), to: CGPoint(x: 0, y: h * 0.35))
            // 太阳
            cg.setFillColor(CGColor(red: 1.0, green: 0.93, blue: 0.78, alpha: 0.95))
            cg.fillEllipse(in: CGRect(x: w * 0.62, y: h * 0.52, width: w * 0.13, height: w * 0.13))
            // 远山两层
            mountainLayer(cg, w: w, h: h, baseY: h * 0.34, amplitude: h * 0.16,
                          color: CGColor(red: 0.22, green: 0.36, blue: 0.32, alpha: 1), seed: 3)
            mountainLayer(cg, w: w, h: h, baseY: h * 0.20, amplitude: h * 0.13,
                          color: CGColor(red: 0.10, green: 0.20, blue: 0.18, alpha: 1), seed: 7)
            // 湖面倒影
            gradient([CGColor(red: 0.10, green: 0.22, blue: 0.22, alpha: 0.0),
                      CGColor(red: 0.05, green: 0.12, blue: 0.13, alpha: 1)],
                     from: CGPoint(x: 0, y: h * 0.2), to: CGPoint(x: 0, y: 0))

        case .cityNight:
            // 夜空
            gradient([CGColor(red: 0.05, green: 0.07, blue: 0.16, alpha: 1),
                      CGColor(red: 0.13, green: 0.16, blue: 0.30, alpha: 1),
                      CGColor(red: 0.25, green: 0.20, blue: 0.28, alpha: 1)],
                     from: CGPoint(x: 0, y: h), to: CGPoint(x: 0, y: h * 0.18))
            // 星星
            var seed: UInt64 = 42
            for _ in 0..<140 {
                let sx = CGFloat((seedRandom(&seed) % 1600)) / 1600 * w
                let sy = h * 0.35 + CGFloat((seedRandom(&seed) % 1000)) / 1000 * h * 0.6
                let r = CGFloat(seedRandom(&seed) % 3) + 1
                cg.setFillColor(CGColor(red: 1, green: 1, blue: 1, alpha: 0.65))
                cg.fillEllipse(in: CGRect(x: sx, y: sy, width: r, height: r))
            }
            // 楼宇剪影
            var x: CGFloat = 0
            while x < w {
                let bw = 40 + CGFloat(seedRandom(&seed) % 90)
                let bh = h * (0.16 + CGFloat(seedRandom(&seed) % 30) / 100)
                cg.setFillColor(CGColor(red: 0.04, green: 0.05, blue: 0.10, alpha: 1))
                cg.fill(CGRect(x: x, y: 0, width: bw, height: bh))
                // 亮窗
                if seedRandom(&seed) % 3 != 0 {
                    let wx = x + bw * 0.25, wy = bh * 0.3
                    cg.setFillColor(CGColor(red: 1.0, green: 0.85, blue: 0.55, alpha: 0.85))
                    cg.fill(CGRect(x: wx, y: wy, width: bw * 0.14, height: bh * 0.09))
                    cg.fill(CGRect(x: wx + bw * 0.34, y: wy + bh * 0.18, width: bw * 0.14, height: bh * 0.09))
                }
                x += bw + 8
            }

        case .minimal:
            // 极简柔和三色渐变
            gradient([CGColor(red: 0.87, green: 0.90, blue: 0.86, alpha: 1),
                      CGColor(red: 0.62, green: 0.73, blue: 0.66, alpha: 1)],
                     from: CGPoint(x: 0, y: h), to: CGPoint(x: w, y: 0))
            // 柔和圆环装饰
            cg.setFillColor(CGColor(red: 0.36, green: 0.55, blue: 0.45, alpha: 0.55))
            cg.fillEllipse(in: CGRect(x: w * 0.55, y: h * 0.42, width: w * 0.30, height: w * 0.30))
            cg.setFillColor(CGColor(red: 0.91, green: 0.86, blue: 0.74, alpha: 0.8))
            cg.fillEllipse(in: CGRect(x: w * 0.18, y: h * 0.18, width: w * 0.16, height: w * 0.16))

        case .cartoon:
            // 卡通插画：随机场景 + 随机布局，每次生成都不重样
            renderCartoonScene(cg, w: w, h: h,
                               seed: UInt64.random(in: 0..<1_000_000))
        }

        guard let image = cg.makeImage() else { return nil }
        return NSImage(cgImage: image, size: size)
    }

    private static func mountainLayer(_ cg: CGContext, w: CGFloat, h: CGFloat,
                                      baseY: CGFloat, amplitude: CGFloat,
                                      color: CGColor, seed: UInt64) {
        var s = seed
        cg.setFillColor(color)
        cg.beginPath()
        cg.move(to: CGPoint(x: 0, y: 0))
        cg.addLine(to: CGPoint(x: 0, y: baseY))
        var x: CGFloat = 0
        var y = baseY
        while x < w {
            let step = 140 + CGFloat(seedRandom(&s) % 120)
            x += step
            y = baseY + CGFloat(seedRandom(&s) % 1000) / 1000 * amplitude - amplitude / 2
            cg.addLine(to: CGPoint(x: x, y: y))
        }
        cg.addLine(to: CGPoint(x: w, y: 0))
        cg.closePath()
        cg.fillPath()
    }

    // MARK: - 卡通插画场景（白天草甸 / 黄昏山谷 / 星空夜晚）

    private static func renderCartoonScene(_ cg: CGContext, w: CGFloat, h: CGFloat, seed: UInt64) {
        var s = seed
        let variant = seedRandom(&s) % 3   // 场景选择
        switch variant {
        case 0: renderCartoonDay(cg, w: w, h: h, seed: &s)
        case 1: renderCartoonDusk(cg, w: w, h: h, seed: &s)
        default: renderCartoonNight(cg, w: w, h: h, seed: &s)
        }
    }

    /// 卡通白天草甸：蓝天白云、圆顶山、绿草坡、树和小花
    private static func renderCartoonDay(_ cg: CGContext, w: CGFloat, h: CGFloat, seed: inout UInt64) {
        // 天空
        cg.drawLinearGradient(
            CGGradient(colorsSpace: nil, colors: [
                CGColor(red: 0.62, green: 0.84, blue: 0.96, alpha: 1),
                CGColor(red: 0.88, green: 0.96, blue: 0.99, alpha: 1)
            ] as CFArray, locations: nil)!,
            start: CGPoint(x: 0, y: h), end: CGPoint(x: 0, y: h * 0.3), options: [])

        // 太阳 + 柔和光晕
        let sunX = w * (0.15 + CGFloat(seedRandom(&seed) % 30) / 100)
        cg.setFillColor(CGColor(red: 1.0, green: 0.95, blue: 0.72, alpha: 0.45))
        cg.fillEllipse(in: CGRect(x: sunX - 60, y: h * 0.68 - 60, width: 240, height: 240))
        cg.setFillColor(CGColor(red: 1.0, green: 0.88, blue: 0.45, alpha: 1))
        cg.fillEllipse(in: CGRect(x: sunX, y: h * 0.68, width: 130, height: 130))

        // 云朵（每组 3-4 个重叠椭圆）
        let cloudCount = 3 + Int(seedRandom(&seed) % 3)
        for _ in 0..<cloudCount {
            let cx = CGFloat(seedRandom(&seed) % UInt64(Int(w)))
            let cy = h * (0.5 + CGFloat(seedRandom(&seed) % 30) / 100)
            let scale = 0.7 + CGFloat(seedRandom(&seed) % 60) / 100
            drawCartoonCloud(cg, center: CGPoint(x: cx, y: cy), scale: scale)
        }

        // 圆顶远山
        cg.setFillColor(CGColor(red: 0.65, green: 0.82, blue: 0.70, alpha: 1))
        cg.fillEllipse(in: CGRect(x: -w * 0.15, y: h * 0.20, width: w * 0.75, height: h * 0.62))
        cg.setFillColor(CGColor(red: 0.55, green: 0.75, blue: 0.62, alpha: 1))
        cg.fillEllipse(in: CGRect(x: w * 0.45, y: h * 0.15, width: w * 0.8, height: h * 0.66))

        // 草坡两层
        cg.setFillColor(CGColor(red: 0.62, green: 0.80, blue: 0.45, alpha: 1))
        cg.fillEllipse(in: CGRect(x: -w * 0.2, y: -h * 0.32, width: w * 1.4, height: h * 0.78))
        cg.setFillColor(CGColor(red: 0.50, green: 0.72, blue: 0.38, alpha: 1))
        cg.fillEllipse(in: CGRect(x: -w * 0.1, y: -h * 0.45, width: w * 1.3, height: h * 0.7))

        // 树
        let treeCount = 2 + Int(seedRandom(&seed) % 2)
        for _ in 0..<treeCount {
            let tx = w * (0.08 + CGFloat(seedRandom(&seed) % 84) / 100)
            let ty = h * (0.05 + CGFloat(seedRandom(&seed) % 14) / 100)
            drawCartoonTree(cg, at: CGPoint(x: tx, y: ty), scale: 0.8 + CGFloat(seedRandom(&seed) % 50) / 100)
        }

        // 小花点缀
        let flowerCount = 6 + Int(seedRandom(&seed) % 8)
        let flowerColors: [[CGFloat]] = [
            [1.0, 0.85, 0.92], [1.0, 0.95, 0.75], [0.95, 0.85, 1.0], [1.0, 1.0, 1.0]
        ]
        for _ in 0..<flowerCount {
            let fx = CGFloat(seedRandom(&seed) % UInt64(Int(w)))
            let fy = h * (0.02 + CGFloat(seedRandom(&seed) % 16) / 100)
            let c = flowerColors[Int(seedRandom(&seed) % 4)]
            cg.setFillColor(CGColor(red: c[0], green: c[1], blue: c[2], alpha: 1))
            cg.fillEllipse(in: CGRect(x: fx - 7, y: fy - 7, width: 14, height: 14))
            cg.setFillColor(CGColor(red: 1.0, green: 0.85, blue: 0.35, alpha: 1))
            cg.fillEllipse(in: CGRect(x: fx - 3, y: fy - 3, width: 6, height: 6))
        }

        // 飞鸟（v 形弧线）
        cg.setStrokeColor(CGColor(red: 0.35, green: 0.45, blue: 0.5, alpha: 0.85))
        cg.setLineWidth(5)
        cg.setLineCap(.round)
        for _ in 0..<(2 + Int(seedRandom(&seed) % 3)) {
            let bx = w * (0.2 + CGFloat(seedRandom(&seed) % 60) / 100)
            let by = h * (0.55 + CGFloat(seedRandom(&seed) % 25) / 100)
            let span: CGFloat = 22
            cg.beginPath()
            cg.move(to: CGPoint(x: bx - span, y: by))
            cg.addQuadCurve(to: CGPoint(x: bx, y: by + 14), control: CGPoint(x: bx - span / 2, y: by + 16))
            cg.addQuadCurve(to: CGPoint(x: bx + span, y: by), control: CGPoint(x: bx + span / 2, y: by + 16))
            cg.strokePath()
        }
    }

    /// 卡通黄昏山谷：暖色渐变、半沉大太阳、群山剪影、飞鸟
    private static func renderCartoonDusk(_ cg: CGContext, w: CGFloat, h: CGFloat, seed: inout UInt64) {
        cg.drawLinearGradient(
            CGGradient(colorsSpace: nil, colors: [
                CGColor(red: 0.42, green: 0.36, blue: 0.62, alpha: 1),
                CGColor(red: 0.97, green: 0.60, blue: 0.55, alpha: 1),
                CGColor(red: 1.00, green: 0.85, blue: 0.63, alpha: 1)
            ] as CFArray, locations: [0, 0.55, 1])!,
            start: CGPoint(x: 0, y: h), end: CGPoint(x: 0, y: h * 0.22), options: [])

        // 大太阳半沉入山
        cg.setFillColor(CGColor(red: 1.0, green: 0.92, blue: 0.55, alpha: 0.35))
        cg.fillEllipse(in: CGRect(x: w * 0.5 - 110, y: h * 0.34 - 110, width: 340, height: 340))
        cg.setFillColor(CGColor(red: 1.0, green: 0.83, blue: 0.48, alpha: 1))
        cg.fillEllipse(in: CGRect(x: w * 0.5, y: h * 0.34, width: 130, height: 130))

        // 群山剪影（多层三角，暖紫色调）
        let layers: [(CGFloat, CGFloat, CGFloat)] = [
            (0.32, 0.72, 0.60),   // y 基线、颜色亮度、高度系数
            (0.22, 0.52, 0.85),
            (0.10, 0.34, 1.05)
        ]
        for (index, layer) in layers.enumerated() {
            var lx: CGFloat = -60
            cg.setFillColor(CGColor(red: 0.45 * layer.1 + 0.08,
                                    green: 0.32 * layer.1 + 0.06,
                                    blue: 0.48 * layer.1 + 0.10, alpha: 1))
            cg.beginPath()
            cg.move(to: CGPoint(x: 0, y: 0))
            cg.addLine(to: CGPoint(x: 0, y: h * (layer.0 + 0.1)))
            while lx < w {
                let peakW = 260 + CGFloat(seedRandom(&seed) % 260)
                let peakH = h * layer.2 * (0.5 + CGFloat(seedRandom(&seed) % 60) / 100)
                cg.addLine(to: CGPoint(x: lx + peakW / 2, y: h * layer.0 + peakH))
                cg.addLine(to: CGPoint(x: lx + peakW, y: h * (layer.0 - 0.04)))
                lx += peakW
                _ = index
            }
            cg.addLine(to: CGPoint(x: w, y: 0))
            cg.closePath()
            cg.fillPath()
        }

        // 飞鸟剪影
        cg.setStrokeColor(CGColor(red: 0.24, green: 0.16, blue: 0.28, alpha: 0.9))
        cg.setLineWidth(6)
        cg.setLineCap(.round)
        for _ in 0..<(3 + Int(seedRandom(&seed) % 3)) {
            let bx = w * (0.15 + CGFloat(seedRandom(&seed) % 70) / 100)
            let by = h * (0.6 + CGFloat(seedRandom(&seed) % 30) / 100)
            let span: CGFloat = 26
            cg.beginPath()
            cg.move(to: CGPoint(x: bx - span, y: by))
            cg.addQuadCurve(to: CGPoint(x: bx, y: by + 16), control: CGPoint(x: bx - span / 2, y: by + 19))
            cg.addQuadCurve(to: CGPoint(x: bx + span, y: by), control: CGPoint(x: bx + span / 2, y: by + 19))
            cg.strokePath()
        }
    }

    /// 卡通星空夜晚：星月、山影、小屋暖窗、萤火虫
    private static func renderCartoonNight(_ cg: CGContext, w: CGFloat, h: CGFloat, seed: inout UInt64) {
        cg.drawLinearGradient(
            CGGradient(colorsSpace: nil, colors: [
                CGColor(red: 0.08, green: 0.11, blue: 0.26, alpha: 1),
                CGColor(red: 0.20, green: 0.16, blue: 0.38, alpha: 1),
                CGColor(red: 0.33, green: 0.24, blue: 0.44, alpha: 1)
            ] as CFArray, locations: nil)!,
            start: CGPoint(x: 0, y: h), end: CGPoint(x: 0, y: h * 0.25), options: [])

        // 星星
        let starCount = 120 + Int(seedRandom(&seed) % 80)
        for _ in 0..<starCount {
            let sx = CGFloat(seedRandom(&seed) % UInt64(Int(w)))
            let sy = h * (0.35 + CGFloat(seedRandom(&seed) % 650) / 1000)
            let r = CGFloat(seedRandom(&seed) % 3) + 1
            cg.setFillColor(CGColor(red: 1, green: 1, blue: 0.95, alpha: 0.5 + CGFloat(seedRandom(&seed) % 40) / 100))
            cg.fillEllipse(in: CGRect(x: sx, y: sy, width: r, height: r))
        }

        // 月牙
        let moonX = w * (0.12 + CGFloat(seedRandom(&seed) % 20) / 100)
        let moonY = h * 0.72
        cg.setFillColor(CGColor(red: 1.0, green: 0.95, blue: 0.75, alpha: 1))
        cg.fillEllipse(in: CGRect(x: moonX, y: moonY, width: 120, height: 120))
        cg.setFillColor(CGColor(red: 0.12, green: 0.14, blue: 0.30, alpha: 1))
        cg.fillEllipse(in: CGRect(x: moonX + 34, y: moonY + 22, width: 108, height: 108))

        // 山影
        cg.setFillColor(CGColor(red: 0.13, green: 0.13, blue: 0.28, alpha: 1))
        cg.beginPath()
        cg.move(to: CGPoint(x: 0, y: 0))
        cg.addLine(to: CGPoint(x: 0, y: h * 0.3))
        cg.addLine(to: CGPoint(x: w * 0.25, y: h * 0.52))
        cg.addLine(to: CGPoint(x: w * 0.5, y: h * 0.26))
        cg.addLine(to: CGPoint(x: w * 0.75, y: h * 0.48))
        cg.addLine(to: CGPoint(x: w, y: h * 0.3))
        cg.addLine(to: CGPoint(x: w, y: 0))
        cg.closePath()
        cg.fillPath()
        cg.setFillColor(CGColor(red: 0.08, green: 0.09, blue: 0.20, alpha: 1))
        cg.fill(CGRect(x: 0, y: 0, width: w, height: h * 0.24))

        // 小屋 + 暖窗
        let hx = w * (0.4 + CGFloat(seedRandom(&seed) % 25) / 100)
        let hy = h * 0.24
        cg.setFillColor(CGColor(red: 0.16, green: 0.12, blue: 0.16, alpha: 1))
        cg.fill(CGRect(x: hx, y: hy, width: 130, height: 95))              // 屋身
        cg.setFillColor(CGColor(red: 0.30, green: 0.12, blue: 0.14, alpha: 1))
        cg.beginPath()                                                     // 屋顶
        cg.move(to: CGPoint(x: hx - 18, y: hy + 95))
        cg.addLine(to: CGPoint(x: hx + 65, y: hy + 150))
        cg.addLine(to: CGPoint(x: hx + 148, y: hy + 95))
        cg.closePath()
        cg.fillPath()
        cg.setFillColor(CGColor(red: 1.0, green: 0.82, blue: 0.42, alpha: 1))
        cg.fill(CGRect(x: hx + 26, y: hy + 26, width: 34, height: 34))     // 暖窗

        // 萤火虫（暖黄小点 + 微光晕）
        let fireflyCount = 10 + Int(seedRandom(&seed) % 10)
        for _ in 0..<fireflyCount {
            let fx = CGFloat(seedRandom(&seed) % UInt64(Int(w)))
            let fy = h * (0.05 + CGFloat(seedRandom(&seed) % 22) / 100)
            cg.setFillColor(CGColor(red: 0.9, green: 1.0, blue: 0.55, alpha: 0.18))
            cg.fillEllipse(in: CGRect(x: fx - 6, y: fy - 6, width: 16, height: 16))
            cg.setFillColor(CGColor(red: 0.95, green: 1.0, blue: 0.6, alpha: 0.95))
            cg.fillEllipse(in: CGRect(x: fx, y: fy, width: 4, height: 4))
        }
    }

    private static func drawCartoonCloud(_ cg: CGContext, center: CGPoint, scale: CGFloat) {
        cg.setFillColor(CGColor(red: 1, green: 1, blue: 1, alpha: 0.92))
        let r1: CGFloat = 46 * scale
        cg.fillEllipse(in: CGRect(x: center.x - r1 * 2, y: center.y, width: r1 * 2, height: r1 * 1.4))
        cg.fillEllipse(in: CGRect(x: center.x - r1 * 0.6, y: center.y + r1 * 0.7, width: r1 * 1.7, height: r1 * 1.5))
        cg.fillEllipse(in: CGRect(x: center.x + r1 * 0.6, y: center.y, width: r1 * 1.6, height: r1 * 1.3))
        cg.fillEllipse(in: CGRect(x: center.x - r1, y: center.y - r1 * 0.4, width: r1 * 2.4, height: r1 * 1.2))
    }

    private static func drawCartoonTree(_ cg: CGContext, at point: CGPoint, scale: CGFloat) {
        // 树干
        cg.setFillColor(CGColor(red: 0.52, green: 0.36, blue: 0.24, alpha: 1))
        cg.fill(CGRect(x: point.x - 10 * scale, y: point.y, width: 20 * scale, height: 90 * scale))
        // 圆冠（三层叠圆）
        cg.setFillColor(CGColor(red: 0.25, green: 0.56, blue: 0.32, alpha: 1))
        cg.fillEllipse(in: CGRect(x: point.x - 75 * scale, y: point.y + 55 * scale, width: 150 * scale, height: 130 * scale))
        cg.setFillColor(CGColor(red: 0.32, green: 0.64, blue: 0.38, alpha: 1))
        cg.fillEllipse(in: CGRect(x: point.x - 60 * scale, y: point.y + 110 * scale, width: 120 * scale, height: 110 * scale))
        cg.setFillColor(CGColor(red: 0.40, green: 0.70, blue: 0.44, alpha: 1))
        cg.fillEllipse(in: CGRect(x: point.x - 45 * scale, y: point.y + 160 * scale, width: 90 * scale, height: 85 * scale))
    }

    private static func seedRandom(_ seed: inout UInt64) -> UInt64 {
        seed = seed &* 6364136223846793005 &+ 1442695040888963407
        return (seed >> 33)
    }
}
