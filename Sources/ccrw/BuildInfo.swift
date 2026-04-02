import Foundation

enum BuildInfo {
    static let bundleName = "Claude Code Rate Watcher"
    static let bundleIdentifier = "com.claude-code-rate-watcher"
    static let fallbackVersion = "0.9.0"

    static var currentVersion: String {
        Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String
            ?? Bundle.main.object(forInfoDictionaryKey: "CFBundleVersion") as? String
            ?? fallbackVersion
    }
}
