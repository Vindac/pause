// 生成应用图标（窗户 + 远方太阳/山 的意象）。
// 用法: swift Scripts/gen_icon.swift <output.iconset 目录>
import AppKit
import CoreGraphics
import Foundation

let outputDir = CommandLine.arguments.count > 1
    ? URL(fileURLWithPath: CommandLine.arguments[1])
    : URL(fileURLWithPath: "Pause.iconset")

try? FileManager.default.createDirectory(at: outputDir, withIntermediateDirectories: true)

let sizes: [(name: String, size: Int)] = [
    ("icon_16x16.png", 16), ("icon_16x16@2x.png", 32),
    ("icon_32_32.png", 32), ("icon_32_32@2x.png", 64),
    ("icon_128x128.png", 128), ("icon_128x128@2x.png", 256),
    ("icon_256x256.png", 256), ("icon_256x256@2x.png", 512),
    ("icon_512x512.png", 512), ("icon_512x512@2x.png", 1024)
]

func renderIcon(size: CGFloat) -> CGImage? {
    let s = size
    guard let ctx = CGContext(
        data: nil, width: Int(s), height: Int(s),
        bitsPerComponent: 8, bytesPerRow: 0,
        space: CGColorSpace(name: CGColorSpace.sRGB)!,
        bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
    ) else { return nil }

    // 画布坐标以 s 为准；比例都相对 1024 设计稿换算
    let u = s / 1024

    // 背景：圆角矩形 + 天空渐变（强调绿 #5B8C72 方向的黄昏色）
    let bgRect = CGRect(x: 0, y: 0, width: s, height: s)
    let radius = 224 * u
    let path = CGPath(roundedRect: bgRect, cornerWidth: radius, cornerHeight: radius, transform: nil)
    ctx.addPath(path)
    ctx.clip()

    let colors = [
        CGColor(red: 0.955, green: 0.88, blue: 0.73, alpha: 1),
        CGColor(red: 0.55, green: 0.68, blue: 0.60, alpha: 1),
        CGColor(red: 0.24, green: 0.38, blue: 0.35, alpha: 1)
    ] as CFArray
    if let gradient = CGGradient(colorsSpace: nil, colors: colors, locations: nil) {
        ctx.drawLinearGradient(gradient,
                               start: CGPoint(x: s * 0.2, y: s * 0.95),
                               end: CGPoint(x: s * 0.8, y: s * 0.15),
                               options: [])
    }

    // 远方太阳
    ctx.setFillColor(CGColor(red: 1.0, green: 0.95, blue: 0.80, alpha: 0.95))
    ctx.fillEllipse(in: CGRect(x: 620 * u, y: 560 * u, width: 200 * u, height: 200 * u))

    // 两层山
    ctx.setFillColor(CGColor(red: 0.30, green: 0.44, blue: 0.38, alpha: 1))
    ctx.beginPath()
    ctx.move(to: CGPoint(x: 0, y: 0))
    ctx.addLine(to: CGPoint(x: 0, y: 470 * u))
    ctx.addLine(to: CGPoint(x: 300 * u, y: 640 * u))
    ctx.addLine(to: CGPoint(x: 560 * u, y: 400 * u))
    ctx.addLine(to: CGPoint(x: 820 * u, y: 560 * u))
    ctx.addLine(to: CGPoint(x: s, y: 440 * u))
    ctx.addLine(to: CGPoint(x: s, y: 0))
    ctx.closePath()
    ctx.fillPath()

    ctx.setFillColor(CGColor(red: 0.14, green: 0.25, blue: 0.23, alpha: 1))
    ctx.beginPath()
    ctx.move(to: CGPoint(x: 0, y: 0))
    ctx.addLine(to: CGPoint(x: 0, y: 260 * u))
    ctx.addLine(to: CGPoint(x: 380 * u, y: 470 * u))
    ctx.addLine(to: CGPoint(x: 720 * u, y: 200 * u))
    ctx.addLine(to: CGPoint(x: s, y: 330 * u))
    ctx.addLine(to: CGPoint(x: s, y: 0))
    ctx.closePath()
    ctx.fillPath()

    // 窗框意象：白色描边圆角矩形 + 十字分格
    let frameRect = CGRect(x: 192 * u, y: 172 * u, width: 640 * u, height: 640 * u)
    let framePath = CGPath(roundedRect: frameRect,
                           cornerWidth: 56 * u, cornerHeight: 56 * u, transform: nil)
    ctx.addPath(framePath)
    ctx.setStrokeColor(CGColor(red: 1, green: 1, blue: 1, alpha: 0.92))
    ctx.setLineWidth(46 * u)
    ctx.strokePath()

    ctx.beginPath()
    ctx.move(to: CGPoint(x: 512 * u, y: frameRect.minY + 10 * u))
    ctx.addLine(to: CGPoint(x: 512 * u, y: frameRect.maxY - 10 * u))
    ctx.move(to: CGPoint(x: frameRect.minX + 10 * u, y: 512 * u))
    ctx.addLine(to: CGPoint(x: frameRect.maxX - 10 * u, y: 512 * u))
    ctx.setStrokeColor(CGColor(red: 1, green: 1, blue: 1, alpha: 0.92))
    ctx.setLineWidth(36 * u)
    ctx.setLineCap(.round)
    ctx.strokePath()

    return ctx.makeImage()
}

func writePNG(_ image: CGImage, to url: URL) {
    guard let dest = CGImageDestinationCreateWithURL(
        url as CFURL, "public.png" as CFString, 1, nil
    ) else { return }
    CGImageDestinationAddImage(dest, image, nil)
    CGImageDestinationFinalize(dest)
}

for entry in sizes {
    if let image = renderIcon(size: CGFloat(entry.size)) {
        writePNG(image, to: outputDir.appendingPathComponent(entry.name))
    } else {
        FileHandle.standardError.write("failed to render \(entry.name)\n".data(using: .utf8)!)
        exit(1)
    }
}
print("iconset written to \(outputDir.path)")
