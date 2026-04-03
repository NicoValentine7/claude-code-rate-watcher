import Foundation

public enum HistoryAggregator {
    public static func hourlyBuckets(from snapshots: [HistorySnapshot]) -> [AggregatedSnapshot] {
        let calendar = Calendar.current
        let grouped = Dictionary(grouping: snapshots) { snapshot in
            calendar.dateInterval(of: .hour, for: snapshot.timestamp)?.start ?? snapshot.timestamp
        }
        return grouped.map { bucketStart, group in
            let bucketEnd = calendar.date(byAdding: .hour, value: 1, to: bucketStart) ?? bucketStart
            return AggregatedSnapshot(
                bucketStart: bucketStart,
                bucketEnd: bucketEnd,
                avgFiveHourPercent: Double(group.map(\.fiveHourPercent).reduce(0, +)) / Double(group.count),
                avgSevenDayPercent: Double(group.map(\.sevenDayPercent).reduce(0, +)) / Double(group.count),
                maxFiveHourPercent: group.map(\.fiveHourPercent).max() ?? 0,
                maxSevenDayPercent: group.map(\.sevenDayPercent).max() ?? 0,
                sampleCount: group.count
            )
        }.sorted { $0.bucketStart < $1.bucketStart }
    }

    public static func dailyBuckets(from hourly: [AggregatedSnapshot]) -> [AggregatedSnapshot] {
        let calendar = Calendar.current
        let grouped = Dictionary(grouping: hourly) { snapshot in
            calendar.startOfDay(for: snapshot.bucketStart)
        }
        return grouped.map { dayStart, group in
            let totalSamples = group.map(\.sampleCount).reduce(0, +)
            let dayEnd = calendar.date(byAdding: .day, value: 1, to: dayStart) ?? dayStart
            return AggregatedSnapshot(
                bucketStart: dayStart,
                bucketEnd: dayEnd,
                avgFiveHourPercent: group.reduce(0.0) { $0 + $1.avgFiveHourPercent * Double($1.sampleCount) } / Double(max(totalSamples, 1)),
                avgSevenDayPercent: group.reduce(0.0) { $0 + $1.avgSevenDayPercent * Double($1.sampleCount) } / Double(max(totalSamples, 1)),
                maxFiveHourPercent: group.map(\.maxFiveHourPercent).max() ?? 0,
                maxSevenDayPercent: group.map(\.maxSevenDayPercent).max() ?? 0,
                sampleCount: totalSamples
            )
        }.sorted { $0.bucketStart < $1.bucketStart }
    }

    public static func downsample(_ points: [ChartDataPoint], maxPoints: Int) -> [ChartDataPoint] {
        guard points.count > maxPoints else { return points }
        let step = Double(points.count) / Double(maxPoints)
        return (0..<maxPoints).map { i in
            let idx = min(Int(Double(i) * step), points.count - 1)
            return points[idx]
        }
    }
}
