import Testing
import Foundation
@testable import CCRWCore

@Suite("HistoryAggregator")
struct HistoryAggregatorTests {

    private func makeSnapshot(
        hour: Int, minute: Int = 0,
        fiveHour: Int, sevenDay: Int = 20
    ) -> HistorySnapshot {
        let calendar = Calendar.current
        let date = calendar.date(bySettingHour: hour, minute: minute, second: 0, of: Date())!
        return HistorySnapshot(
            timestamp: date,
            fiveHourPercent: fiveHour,
            sevenDayPercent: sevenDay,
            fiveHourWeightedTokens: UInt64(fiveHour) * 250_000,
            sevenDayWeightedTokens: UInt64(sevenDay) * 2_250_000,
            messageCount: 10,
            weeklyMessageCount: 50,
            source: .localEstimate,
            isLive: false
        )
    }

    @Test("hourlyBuckets groups snapshots by hour and computes averages")
    func hourlyBucketsGroupByHour() {
        let snapshots = [
            makeSnapshot(hour: 10, minute: 0, fiveHour: 40),
            makeSnapshot(hour: 10, minute: 15, fiveHour: 60),
            makeSnapshot(hour: 10, minute: 30, fiveHour: 80),
            makeSnapshot(hour: 11, minute: 0, fiveHour: 30),
        ]

        let buckets = HistoryAggregator.hourlyBuckets(from: snapshots)

        #expect(buckets.count == 2)
        let first = buckets[0]
        #expect(first.sampleCount == 3)
        #expect(first.avgFiveHourPercent == 60.0)
        #expect(first.maxFiveHourPercent == 80)
        let second = buckets[1]
        #expect(second.sampleCount == 1)
        #expect(second.avgFiveHourPercent == 30.0)
    }

    @Test("dailyBuckets computes weighted average from hourly data")
    func dailyBucketsWeightedAverage() {
        let calendar = Calendar.current
        let today = calendar.startOfDay(for: Date())
        let hourly = [
            AggregatedSnapshot(
                bucketStart: today,
                bucketEnd: calendar.date(byAdding: .hour, value: 1, to: today)!,
                avgFiveHourPercent: 40.0,
                avgSevenDayPercent: 20.0,
                maxFiveHourPercent: 50,
                maxSevenDayPercent: 25,
                sampleCount: 100
            ),
            AggregatedSnapshot(
                bucketStart: calendar.date(byAdding: .hour, value: 1, to: today)!,
                bucketEnd: calendar.date(byAdding: .hour, value: 2, to: today)!,
                avgFiveHourPercent: 80.0,
                avgSevenDayPercent: 40.0,
                maxFiveHourPercent: 90,
                maxSevenDayPercent: 45,
                sampleCount: 100
            ),
        ]

        let daily = HistoryAggregator.dailyBuckets(from: hourly)

        #expect(daily.count == 1)
        #expect(daily[0].avgFiveHourPercent == 60.0)
        #expect(daily[0].maxFiveHourPercent == 90)
        #expect(daily[0].sampleCount == 200)
    }

    @Test("empty input returns empty output")
    func emptyInputReturnsEmpty() {
        #expect(HistoryAggregator.hourlyBuckets(from: []).isEmpty)
        #expect(HistoryAggregator.dailyBuckets(from: []).isEmpty)
    }

    @Test("downsample reduces points to max count")
    func downsampleReducesPoints() {
        let points = (0..<1000).map { i in
            ChartDataPoint(date: Date().addingTimeInterval(Double(i) * 30), fiveHour: Double(i % 100), sevenDay: 20)
        }
        let result = HistoryAggregator.downsample(points, maxPoints: 100)
        #expect(result.count == 100)
    }

    @Test("downsample preserves small arrays")
    func downsamplePreservesSmall() {
        let points = [
            ChartDataPoint(date: Date(), fiveHour: 50, sevenDay: 20),
            ChartDataPoint(date: Date().addingTimeInterval(30), fiveHour: 55, sevenDay: 22),
        ]
        let result = HistoryAggregator.downsample(points, maxPoints: 100)
        #expect(result.count == 2)
    }
}
