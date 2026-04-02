import Foundation

public enum SnapshotSource: String, Codable, CaseIterable, Sendable {
    case liveAPI = "live_api"
    case statusline
    case staleFallback = "stale_fallback"
    case unavailable
    case localEstimate = "local_estimate"
}

public struct RateLimitSnapshot: Equatable, Sendable {
    public var fiveHourPercent: Int?
    public var sevenDayPercent: Int?
    public var fiveHourResetsAt: Date?
    public var sevenDayResetsAt: Date?
    public var isLive: Bool
    public var authMissing: Bool
    public var errorMessage: String?
    public var errorDetail: String?
    public var retryCount: Int
    public var retryAt: Date?
    public var suggestRelogin: Bool
    public var source: SnapshotSource

    public init(
        fiveHourPercent: Int? = nil,
        sevenDayPercent: Int? = nil,
        fiveHourResetsAt: Date? = nil,
        sevenDayResetsAt: Date? = nil,
        isLive: Bool = false,
        authMissing: Bool = false,
        errorMessage: String? = nil,
        errorDetail: String? = nil,
        retryCount: Int = 0,
        retryAt: Date? = nil,
        suggestRelogin: Bool = false,
        source: SnapshotSource = .unavailable
    ) {
        self.fiveHourPercent = fiveHourPercent
        self.sevenDayPercent = sevenDayPercent
        self.fiveHourResetsAt = fiveHourResetsAt
        self.sevenDayResetsAt = sevenDayResetsAt
        self.isLive = isLive
        self.authMissing = authMissing
        self.errorMessage = errorMessage
        self.errorDetail = errorDetail
        self.retryCount = retryCount
        self.retryAt = retryAt
        self.suggestRelogin = suggestRelogin
        self.source = source
    }

    public static let empty = RateLimitSnapshot()

    public func staleFallback(using good: RateLimitSnapshot) -> RateLimitSnapshot {
        var fallback = good
        fallback.isLive = false
        fallback.authMissing = authMissing
        fallback.errorMessage = errorMessage
        fallback.errorDetail = errorDetail
        fallback.retryCount = retryCount
        fallback.retryAt = retryAt
        fallback.suggestRelogin = suggestRelogin
        fallback.source = .staleFallback
        return fallback
    }
}

public struct UsageSummary: Equatable, Sendable {
    public var totalInputTokens: UInt64
    public var totalOutputTokens: UInt64
    public var totalCacheCreationTokens: UInt64
    public var totalCacheReadTokens: UInt64
    public var resetTime: Date?
    public var messageCount: Int
    public var usagePercent: Int
    public var weeklyInputTokens: UInt64
    public var weeklyOutputTokens: UInt64
    public var weeklyCacheCreationTokens: UInt64
    public var weeklyCacheReadTokens: UInt64
    public var weeklyResetTime: Date?
    public var weeklyMessageCount: Int
    public var weeklyUsagePercent: Int

    public init(
        totalInputTokens: UInt64 = 0,
        totalOutputTokens: UInt64 = 0,
        totalCacheCreationTokens: UInt64 = 0,
        totalCacheReadTokens: UInt64 = 0,
        resetTime: Date? = nil,
        messageCount: Int = 0,
        usagePercent: Int = 0,
        weeklyInputTokens: UInt64 = 0,
        weeklyOutputTokens: UInt64 = 0,
        weeklyCacheCreationTokens: UInt64 = 0,
        weeklyCacheReadTokens: UInt64 = 0,
        weeklyResetTime: Date? = nil,
        weeklyMessageCount: Int = 0,
        weeklyUsagePercent: Int = 0
    ) {
        self.totalInputTokens = totalInputTokens
        self.totalOutputTokens = totalOutputTokens
        self.totalCacheCreationTokens = totalCacheCreationTokens
        self.totalCacheReadTokens = totalCacheReadTokens
        self.resetTime = resetTime
        self.messageCount = messageCount
        self.usagePercent = usagePercent
        self.weeklyInputTokens = weeklyInputTokens
        self.weeklyOutputTokens = weeklyOutputTokens
        self.weeklyCacheCreationTokens = weeklyCacheCreationTokens
        self.weeklyCacheReadTokens = weeklyCacheReadTokens
        self.weeklyResetTime = weeklyResetTime
        self.weeklyMessageCount = weeklyMessageCount
        self.weeklyUsagePercent = weeklyUsagePercent
    }

    public static let empty = UsageSummary()

    public var totalTokens: UInt64 {
        totalInputTokens + totalOutputTokens + totalCacheCreationTokens + totalCacheReadTokens
    }

    public var weeklyTotalTokens: UInt64 {
        weeklyInputTokens + weeklyOutputTokens + weeklyCacheCreationTokens + weeklyCacheReadTokens
    }

    public func effectiveFiveHourPercent(using snapshot: RateLimitSnapshot) -> Int {
        snapshot.fiveHourPercent ?? usagePercent
    }

    public func effectiveSevenDayPercent(using snapshot: RateLimitSnapshot) -> Int {
        snapshot.sevenDayPercent ?? weeklyUsagePercent
    }
}

public enum AuthStatus: String, Equatable, Sendable {
    case checking
    case authenticated
    case missing
    case actionRequired
    case refreshing
}

public struct AuthState: Equatable, Sendable {
    public var status: AuthStatus
    public var detail: String?
    public var lastError: String?
    public var isBusy: Bool

    public init(
        status: AuthStatus = .checking,
        detail: String? = nil,
        lastError: String? = nil,
        isBusy: Bool = false
    ) {
        self.status = status
        self.detail = detail
        self.lastError = lastError
        self.isBusy = isBusy
    }
}

public enum UpdatePhase: String, Equatable, Sendable {
    case idle
    case checking
    case available
    case upToDate
    case unsupported
    case failed
}

public struct UpdateState: Equatable, Sendable {
    public var phase: UpdatePhase
    public var currentVersion: String
    public var availableVersion: String?
    public var message: String?

    public init(
        phase: UpdatePhase = .idle,
        currentVersion: String,
        availableVersion: String? = nil,
        message: String? = nil
    ) {
        self.phase = phase
        self.currentVersion = currentVersion
        self.availableVersion = availableVersion
        self.message = message
    }
}

public struct AppDiagnostics: Equatable, Sendable {
    public var lastRefresh: Date?
    public var sourceDescription: String
    public var statuslineInstalled: Bool
    public var statuslineFresh: Bool
    public var sessionRecordCount: Int
    public var manualRefreshCooldownRemaining: Int
    public var lastError: String?

    public init(
        lastRefresh: Date? = nil,
        sourceDescription: String = "Unavailable",
        statuslineInstalled: Bool = false,
        statuslineFresh: Bool = false,
        sessionRecordCount: Int = 0,
        manualRefreshCooldownRemaining: Int = 0,
        lastError: String? = nil
    ) {
        self.lastRefresh = lastRefresh
        self.sourceDescription = sourceDescription
        self.statuslineInstalled = statuslineInstalled
        self.statuslineFresh = statuslineFresh
        self.sessionRecordCount = sessionRecordCount
        self.manualRefreshCooldownRemaining = manualRefreshCooldownRemaining
        self.lastError = lastError
    }
}

public enum CCRWFormatting {
    public static func shortResetText(for reset: Date, now: Date = Date()) -> String {
        let remaining = max(0, Int(reset.timeIntervalSince(now)))
        guard remaining > 0 else {
            return "Window clear"
        }
        let hours = remaining / 3600
        let minutes = (remaining % 3600) / 60
        return String(format: "Resets in: %dh %02dm", hours, minutes)
    }

    public static func longResetText(for reset: Date, now: Date = Date()) -> String {
        let remaining = max(0, Int(reset.timeIntervalSince(now)))
        guard remaining > 0 else {
            return "Window clear"
        }
        let days = remaining / 86_400
        let hours = (remaining % 86_400) / 3600
        let minutes = (remaining % 3600) / 60
        if days > 0 {
            return String(format: "Resets in: %dd %dh %02dm", days, hours, minutes)
        }
        return String(format: "Resets in: %dh %02dm", hours, minutes)
    }

    public static func compactTimestamp(_ date: Date?) -> String {
        guard let date else {
            return "Never"
        }
        let formatter = DateFormatter()
        formatter.dateStyle = .none
        formatter.timeStyle = .short
        return formatter.string(from: date)
    }
}
