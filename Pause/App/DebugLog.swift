import Foundation

/// 临时调试日志：写入 /tmp/pause_debug.log，用于定位设置页按钮交互问题。
/// 问题解决后可整体移除。
enum DebugLog {
    static let url = URL(fileURLWithPath: "/tmp/pause_debug.log")

    static func log(_ message: String) {
        let line = "\(Date()) [\(Thread.current.name ?? "main")] \(message)\n"
        let data = Data(line.utf8)
        if let handle = try? FileHandle(forWritingTo: url) {
            handle.seekToEndOfFile()
            handle.write(data)
            try? handle.close()
        } else {
            try? data.write(to: url)
        }
    }
}
