import Foundation

public actor HistoryStore {
    public static let defaultBaseDirectory: URL = {
        FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".claude-rate-watcher", isDirectory: true)
            .appendingPathComponent("history", isDirectory: true)
    }()

    public static let rawRetentionDays = 7
    public static let hourlyRetentionDays = 90

    private let baseDir: URL
    private var fileHandle: FileHandle?
    private var currentFileName: String?
    private let encoder: JSONEncoder
    private let decoder: JSONDecoder
    private let dateFormatter: DateFormatter

    public init(baseDirectory: URL = HistoryStore.defaultBaseDirectory) {
        self.baseDir = baseDirectory
        self.encoder = JSONEncoder()
        self.encoder.dateEncodingStrategy = .iso8601
        self.decoder = JSONDecoder()
        self.decoder.dateDecodingStrategy = .iso8601
        self.dateFormatter = DateFormatter()
        self.dateFormatter.dateFormat = "yyyy-MM-dd"
    }

    deinit {
        fileHandle?.closeFile()
    }

    // MARK: - Write

    public func append(_ snapshot: HistorySnapshot) throws {
        let fileName = "snapshots-\(dateFormatter.string(from: snapshot.timestamp)).jsonl"
        if fileName != currentFileName {
            try rotateFileHandle(to: fileName)
        }
        let data = try encoder.encode(snapshot)
        fileHandle?.write(data)
        fileHandle?.write(Data("\n".utf8))
    }

    // MARK: - Read

    public func snapshots(from start: Date, to end: Date) throws -> [HistorySnapshot] {
        let calendar = Calendar.current
        var date = calendar.startOfDay(for: start)
        let endDay = calendar.startOfDay(for: end)
        var results: [HistorySnapshot] = []

        while date <= endDay {
            let fileName = "snapshots-\(dateFormatter.string(from: date)).jsonl"
            let filePath = baseDir.appendingPathComponent(fileName)
            if let lines = try? readLines(from: filePath) {
                for line in lines {
                    guard !line.isEmpty, let data = line.data(using: .utf8) else { continue }
                    guard let snapshot = try? decoder.decode(HistorySnapshot.self, from: data) else { continue }
                    if snapshot.timestamp >= start && snapshot.timestamp <= end {
                        results.append(snapshot)
                    }
                }
            }
            date = calendar.date(byAdding: .day, value: 1, to: date) ?? endDay.addingTimeInterval(1)
        }
        return results.sorted { $0.timestamp < $1.timestamp }
    }

    public func hourlyAggregates(from start: Date, to end: Date) throws -> [AggregatedSnapshot] {
        let monthFormatter = DateFormatter()
        monthFormatter.dateFormat = "yyyy-MM"

        let calendar = Calendar.current
        var month = calendar.dateInterval(of: .month, for: start)?.start ?? start
        let endMonth = calendar.dateInterval(of: .month, for: end)?.start ?? end
        var results: [AggregatedSnapshot] = []

        while month <= endMonth {
            let fileName = "hourly-\(monthFormatter.string(from: month)).jsonl"
            let filePath = baseDir.appendingPathComponent(fileName)
            if let lines = try? readLines(from: filePath) {
                for line in lines {
                    guard !line.isEmpty, let data = line.data(using: .utf8) else { continue }
                    guard let agg = try? decoder.decode(AggregatedSnapshot.self, from: data) else { continue }
                    if agg.bucketStart >= start && agg.bucketStart <= end {
                        results.append(agg)
                    }
                }
            }
            month = calendar.date(byAdding: .month, value: 1, to: month) ?? endMonth.addingTimeInterval(1)
        }
        return results.sorted { $0.bucketStart < $1.bucketStart }
    }

    public func dailyAggregates(from start: Date, to end: Date) throws -> [AggregatedSnapshot] {
        let yearFormatter = DateFormatter()
        yearFormatter.dateFormat = "yyyy"

        let calendar = Calendar.current
        let startYear = calendar.component(.year, from: start)
        let endYear = calendar.component(.year, from: end)
        var results: [AggregatedSnapshot] = []

        for year in startYear...endYear {
            let fileName = "daily-\(year).jsonl"
            let filePath = baseDir.appendingPathComponent(fileName)
            if let lines = try? readLines(from: filePath) {
                for line in lines {
                    guard !line.isEmpty, let data = line.data(using: .utf8) else { continue }
                    guard let agg = try? decoder.decode(AggregatedSnapshot.self, from: data) else { continue }
                    if agg.bucketStart >= start && agg.bucketStart <= end {
                        results.append(agg)
                    }
                }
            }
        }
        return results.sorted { $0.bucketStart < $1.bucketStart }
    }

    // MARK: - Chart Data

    public func chartData(for range: HistoryTimeRange) async throws -> [ChartDataPoint] {
        let now = Date()
        switch range {
        case .day:
            let raw = try snapshots(from: now.addingTimeInterval(-86_400), to: now)
            let points = raw.map { ChartDataPoint(date: $0.timestamp, fiveHour: Double($0.fiveHourPercent), sevenDay: Double($0.sevenDayPercent)) }
            return HistoryAggregator.downsample(points, maxPoints: 288)
        case .week:
            let raw = try snapshots(from: now.addingTimeInterval(-604_800), to: now)
            if raw.isEmpty {
                let hourly = try hourlyAggregates(from: now.addingTimeInterval(-604_800), to: now)
                return hourly.map { ChartDataPoint(date: $0.bucketStart, fiveHour: $0.avgFiveHourPercent, sevenDay: $0.avgSevenDayPercent) }
            }
            let hourly = HistoryAggregator.hourlyBuckets(from: raw)
            return hourly.map { ChartDataPoint(date: $0.bucketStart, fiveHour: $0.avgFiveHourPercent, sevenDay: $0.avgSevenDayPercent) }
        case .month:
            let hourly = try hourlyAggregates(from: now.addingTimeInterval(-2_592_000), to: now)
            if hourly.isEmpty {
                let daily = try dailyAggregates(from: now.addingTimeInterval(-2_592_000), to: now)
                return daily.map { ChartDataPoint(date: $0.bucketStart, fiveHour: $0.avgFiveHourPercent, sevenDay: $0.avgSevenDayPercent) }
            }
            let daily = HistoryAggregator.dailyBuckets(from: hourly)
            return daily.map { ChartDataPoint(date: $0.bucketStart, fiveHour: $0.avgFiveHourPercent, sevenDay: $0.avgSevenDayPercent) }
        }
    }

    // MARK: - Maintenance

    public func runMaintenance(now: Date = Date()) throws {
        try ensureDirectory()
        try aggregateOldSnapshots(now: now)
        try rotateOldFiles(now: now)
    }

    // MARK: - Private

    private func aggregateOldSnapshots(now: Date) throws {
        let calendar = Calendar.current
        let cutoff = calendar.date(byAdding: .day, value: -1, to: calendar.startOfDay(for: now)) ?? now

        let fm = FileManager.default
        guard let files = try? fm.contentsOfDirectory(at: baseDir, includingPropertiesForKeys: nil) else { return }

        let monthFormatter = DateFormatter()
        monthFormatter.dateFormat = "yyyy-MM"

        for file in files where file.lastPathComponent.hasPrefix("snapshots-") && file.pathExtension == "jsonl" {
            let datePart = file.deletingPathExtension().lastPathComponent.replacingOccurrences(of: "snapshots-", with: "")
            guard let fileDate = dateFormatter.date(from: datePart), fileDate < cutoff else { continue }

            let lines = (try? readLines(from: file)) ?? []
            var snaps: [HistorySnapshot] = []
            for line in lines {
                guard !line.isEmpty, let data = line.data(using: .utf8) else { continue }
                if let snap = try? decoder.decode(HistorySnapshot.self, from: data) {
                    snaps.append(snap)
                }
            }
            guard !snaps.isEmpty else { continue }

            let hourly = HistoryAggregator.hourlyBuckets(from: snaps)
            let monthKey = monthFormatter.string(from: fileDate)
            let hourlyFile = baseDir.appendingPathComponent("hourly-\(monthKey).jsonl")
            try appendAggregates(hourly, to: hourlyFile)
        }
    }

    private func rotateOldFiles(now: Date) throws {
        let calendar = Calendar.current
        let rawCutoff = calendar.date(byAdding: .day, value: -Self.rawRetentionDays, to: now) ?? now
        let hourlyCutoff = calendar.date(byAdding: .day, value: -Self.hourlyRetentionDays, to: now) ?? now

        let fm = FileManager.default
        guard let files = try? fm.contentsOfDirectory(at: baseDir, includingPropertiesForKeys: nil) else { return }

        for file in files {
            let name = file.lastPathComponent
            if name.hasPrefix("snapshots-") {
                let datePart = file.deletingPathExtension().lastPathComponent.replacingOccurrences(of: "snapshots-", with: "")
                if let fileDate = dateFormatter.date(from: datePart), fileDate < rawCutoff {
                    try? fm.removeItem(at: file)
                }
            }
        }

        let monthFormatter = DateFormatter()
        monthFormatter.dateFormat = "yyyy-MM"

        for file in files where file.lastPathComponent.hasPrefix("hourly-") {
            let datePart = file.deletingPathExtension().lastPathComponent.replacingOccurrences(of: "hourly-", with: "")
            if let monthDate = monthFormatter.date(from: datePart), monthDate < hourlyCutoff {
                let hourly = (try? readAndDecodeAggregates(from: file)) ?? []
                if !hourly.isEmpty {
                    let daily = HistoryAggregator.dailyBuckets(from: hourly)
                    let year = calendar.component(.year, from: monthDate)
                    let dailyFile = baseDir.appendingPathComponent("daily-\(year).jsonl")
                    try? appendAggregates(daily, to: dailyFile)
                }
                try? fm.removeItem(at: file)
            }
        }
    }

    private func rotateFileHandle(to fileName: String) throws {
        fileHandle?.closeFile()
        try ensureDirectory()
        let path = baseDir.appendingPathComponent(fileName)
        if !FileManager.default.fileExists(atPath: path.path) {
            FileManager.default.createFile(atPath: path.path, contents: nil)
        }
        fileHandle = try FileHandle(forWritingTo: path)
        fileHandle?.seekToEndOfFile()
        currentFileName = fileName
    }

    private func ensureDirectory() throws {
        try FileManager.default.createDirectory(at: baseDir, withIntermediateDirectories: true)
    }

    private func readLines(from url: URL) throws -> [String] {
        let content = try String(contentsOf: url, encoding: .utf8)
        return content.components(separatedBy: .newlines)
    }

    private func appendAggregates(_ aggregates: [AggregatedSnapshot], to file: URL) throws {
        if !FileManager.default.fileExists(atPath: file.path) {
            FileManager.default.createFile(atPath: file.path, contents: nil)
        }
        let handle = try FileHandle(forWritingTo: file)
        handle.seekToEndOfFile()
        for agg in aggregates {
            let data = try encoder.encode(agg)
            handle.write(data)
            handle.write(Data("\n".utf8))
        }
        handle.closeFile()
    }

    private func readAndDecodeAggregates(from url: URL) throws -> [AggregatedSnapshot] {
        let lines = try readLines(from: url)
        return lines.compactMap { line in
            guard !line.isEmpty, let data = line.data(using: .utf8) else { return nil }
            return try? decoder.decode(AggregatedSnapshot.self, from: data)
        }
    }
}
