import Foundation

/// 应用元信息。版本号优先读运行时 Bundle（.app 内的 Info.plist）；
/// 裸二进制（swift run）读不到 Info.plist 时回退编译期常量，两者需与
/// Info.plist 的 CFBundleShortVersionString 保持一致。
enum AppInfo {
    static let fallbackVersion = "1.0.0"

    static let version: String =
        Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String
        ?? fallbackVersion
}
