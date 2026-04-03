import Foundation

// MARK: - History Snapshot

public struct HistorySnapshot: Codable, Sendable, Equatable {
    public let id: UUID
    public let timestamp: Date
    public let fiveHourPercent: Int
    public let sevenDayPercent: Int
    public let fiveHourWeightedTokens: UInt64
    public let sevenDayWeightedTokens: UInt64
    public let messageCount: Int
    public let weeklyMessageCount: Int
    public let source: SnapshotSource
    public let isLive: Bool

    public init(
        id: UUID = UUID(),
        timestamp: Date = Date(),
        fiveHourPercent: Int,
        sevenDayPercent: Int,
        fiveHourWeightedTokens: UInt64,
        sevenDayWeightedTokens: UInt64,
        messageCount: Int,
        weeklyMessageCount: Int,
        source: SnapshotSource,
        isLive: Bool
    ) {
        self.id = id
        self.timestamp = timestamp
        self.fiveHourPercent = fiveHourPercent
        self.sevenDayPercent = sevenDayPercent
        self.fiveHourWeightedTokens = fiveHourWeightedTokens
        self.sevenDayWeightedTokens = sevenDayWeightedTokens
        self.messageCount = messageCount
        self.weeklyMessageCount = weeklyMessageCount
        self.source = source
        self.isLive = isLive
    }
}

// MARK: - Aggregated Snapshot

public struct AggregatedSnapshot: Codable, Sendable, Equatable {
    public let bucketStart: Date
    public let bucketEnd: Date
    public let avgFiveHourPercent: Double
    public let avgSevenDayPercent: Double
    public let maxFiveHourPercent: Int
    public let maxSevenDayPercent: Int
    public let sampleCount: Int

    public init(
        bucketStart: Date,
        bucketEnd: Date,
        avgFiveHourPercent: Double,
        avgSevenDayPercent: Double,
        maxFiveHourPercent: Int,
        maxSevenDayPercent: Int,
        sampleCount: Int
    ) {
        self.bucketStart = bucketStart
        self.bucketEnd = bucketEnd
        self.avgFiveHourPercent = avgFiveHourPercent
        self.avgSevenDayPercent = avgSevenDayPercent
        self.maxFiveHourPercent = maxFiveHourPercent
        self.maxSevenDayPercent = maxSevenDayPercent
        self.sampleCount = sampleCount
    }
}

// MARK: - Chart Data

public enum HistoryTimeRange: String, CaseIterable, Sendable {
    case day = "24H"
    case week = "7D"
    case month = "30D"
}

public struct ChartDataPoint: Sendable, Equatable {
    public let date: Date
    public let fiveHour: Double
    public let sevenDay: Double

    public init(date: Date, fiveHour: Double, sevenDay: Double) {
        self.date = date
        self.fiveHour = fiveHour
        self.sevenDay = sevenDay
    }
}
