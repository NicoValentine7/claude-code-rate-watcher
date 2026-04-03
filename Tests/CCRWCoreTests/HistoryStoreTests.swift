import Testing
import Foundation
@testable import CCRWCore

@Suite("HistoryStore")
struct HistoryStoreTests {

    private func makeTempDir() throws -> URL {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("ccrw-test-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        return url
    }

    private func cleanup(_ url: URL) {
        try? FileManager.default.removeItem(at: url)
    }

    private func makeSnapshot(
        date: Date = Date(),
        fiveHour: Int = 45,
        sevenDay: Int = 20
    ) -> HistorySnapshot {
        HistorySnapshot(
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

    @Test("append and read back snapshots")
    func appendAndReadBack() async throws {
        let dir = try makeTempDir()
        defer { cleanup(dir) }
        let store = HistoryStore(baseDirectory: dir)

        let now = Date()
        let s1 = makeSnapshot(date: now, fiveHour: 30)
        let s2 = makeSnapshot(date: now.addingTimeInterval(30), fiveHour: 40)
        let s3 = makeSnapshot(date: now.addingTimeInterval(60), fiveHour: 50)

        try await store.append(s1)
        try await store.append(s2)
        try await store.append(s3)

        let results = try await store.snapshots(from: now.addingTimeInterval(-1), to: now.addingTimeInterval(61))
        #expect(results.count == 3)
        #expect(results[0].fiveHourPercent == 30)
        #expect(results[2].fiveHourPercent == 50)
    }

    @Test("read returns only requested time range")
    func readReturnsOnlyRequestedRange() async throws {
        let dir = try makeTempDir()
        defer { cleanup(dir) }
        let store = HistoryStore(baseDirectory: dir)

        let now = Date()
        let early = makeSnapshot(date: now.addingTimeInterval(-100), fiveHour: 10)
        let mid = makeSnapshot(date: now, fiveHour: 50)
        let late = makeSnapshot(date: now.addingTimeInterval(100), fiveHour: 90)

        try await store.append(early)
        try await store.append(mid)
        try await store.append(late)

        let results = try await store.snapshots(from: now.addingTimeInterval(-10), to: now.addingTimeInterval(10))
        #expect(results.count == 1)
        #expect(results[0].fiveHourPercent == 50)
    }

    @Test("append creates directory if missing")
    func appendCreatesDirectory() async throws {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("ccrw-nonexistent-\(UUID().uuidString)", isDirectory: true)
        defer { cleanup(dir) }
        let store = HistoryStore(baseDirectory: dir)

        try await store.append(makeSnapshot())

        #expect(FileManager.default.fileExists(atPath: dir.path))
    }

    @Test("empty history returns empty array")
    func emptyHistoryReturnsEmpty() async throws {
        let dir = try makeTempDir()
        defer { cleanup(dir) }
        let store = HistoryStore(baseDirectory: dir)

        let results = try await store.snapshots(from: Date().addingTimeInterval(-3600), to: Date())
        #expect(results.isEmpty)
    }

    @Test("chartData returns empty for no data")
    func chartDataEmptyForNoData() async throws {
        let dir = try makeTempDir()
        defer { cleanup(dir) }
        let store = HistoryStore(baseDirectory: dir)

        let data = try await store.chartData(for: .day)
        #expect(data.isEmpty)
    }

    @Test("chartData returns points after appending")
    func chartDataReturnsPoints() async throws {
        let dir = try makeTempDir()
        defer { cleanup(dir) }
        let store = HistoryStore(baseDirectory: dir)

        let now = Date()
        for i in 0..<5 {
            try await store.append(makeSnapshot(
                date: now.addingTimeInterval(Double(i) * -600),
                fiveHour: 40 + i * 5
            ))
        }

        let data = try await store.chartData(for: .day)
        #expect(data.count == 5)
    }

    @Test("runMaintenance does not crash on empty directory")
    func maintenanceOnEmptyDir() async throws {
        let dir = try makeTempDir()
        defer { cleanup(dir) }
        let store = HistoryStore(baseDirectory: dir)

        try await store.runMaintenance()
    }
}
