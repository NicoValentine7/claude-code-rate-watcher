import Foundation

public enum UsageCalculator {
    public static let fiveHourWindowHours: Int = 5
    public static let weeklyWindowHours: Int = 7 * 24
    public static let estimatedLimit5h: UInt64 = 25_000_000
    public static let estimatedLimitWeekly: UInt64 = 225_000_000

    private struct WindowStats {
        var input: UInt64 = 0
        var output: UInt64 = 0
        var cacheCreation: UInt64 = 0
        var cacheRead: UInt64 = 0
        var messageCount: Int = 0
        var oldestTimestamp: Date?
    }

    public static func calculateUsage(records: [UsageRecord], now: Date = Date()) -> UsageSummary {
        let fiveHourStats = windowStats(
            records: records,
            windowStart: now.addingTimeInterval(TimeInterval(-fiveHourWindowHours * 3600))
        )
        let weeklyStats = windowStats(
            records: records,
            windowStart: now.addingTimeInterval(TimeInterval(-weeklyWindowHours * 3600))
        )

        let fiveHourReset = fiveHourStats.oldestTimestamp?
            .addingTimeInterval(TimeInterval(fiveHourWindowHours * 3600))
        let weeklyReset = weeklyStats.oldestTimestamp?
            .addingTimeInterval(TimeInterval(weeklyWindowHours * 3600))

        let fiveHourPercent = min(100, Int((Double(weightedTokens(fiveHourStats)) / Double(estimatedLimit5h)) * 100.0))
        let weeklyPercent = min(100, Int((Double(weightedTokens(weeklyStats)) / Double(estimatedLimitWeekly)) * 100.0))

        return UsageSummary(
            totalInputTokens: fiveHourStats.input,
            totalOutputTokens: fiveHourStats.output,
            totalCacheCreationTokens: fiveHourStats.cacheCreation,
            totalCacheReadTokens: fiveHourStats.cacheRead,
            resetTime: fiveHourReset,
            messageCount: fiveHourStats.messageCount,
            usagePercent: fiveHourPercent,
            weeklyInputTokens: weeklyStats.input,
            weeklyOutputTokens: weeklyStats.output,
            weeklyCacheCreationTokens: weeklyStats.cacheCreation,
            weeklyCacheReadTokens: weeklyStats.cacheRead,
            weeklyResetTime: weeklyReset,
            weeklyMessageCount: weeklyStats.messageCount,
            weeklyUsagePercent: weeklyPercent
        )
    }

    public static func loadSummary(homeDirectory: URL? = FileManager.default.homeDirectoryForCurrentUser, now: Date = Date()) -> (summary: UsageSummary, records: [UsageRecord]) {
        let records = SessionParser.loadAllSessions(rootDirectory: SessionParser.claudeProjectsDirectory(homeDirectory: homeDirectory))
        return (calculateUsage(records: records, now: now), records)
    }

    private static func windowStats(records: [UsageRecord], windowStart: Date) -> WindowStats {
        let inWindow = records.filter { $0.timestamp >= windowStart }
        return WindowStats(
            input: inWindow.reduce(0) { $0 + $1.usage.inputTokens },
            output: inWindow.reduce(0) { $0 + $1.usage.outputTokens },
            cacheCreation: inWindow.reduce(0) { $0 + $1.usage.cacheCreationInputTokens },
            cacheRead: inWindow.reduce(0) { $0 + $1.usage.cacheReadInputTokens },
            messageCount: inWindow.count,
            oldestTimestamp: inWindow.map(\.timestamp).min()
        )
    }

    private static func weightedTokens(_ stats: WindowStats) -> UInt64 {
        stats.input + (stats.output * 5) + stats.cacheCreation + (stats.cacheRead / 10)
    }
}
