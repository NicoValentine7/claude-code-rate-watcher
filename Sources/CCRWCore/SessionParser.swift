import Foundation

private struct JournalEntry: Decodable {
    let type: String
    let timestamp: String?
    let message: MessageData?
}

private struct MessageData: Decodable {
    let id: String?
    let role: String?
    let usage: UsageData?
}

public struct UsageData: Codable, Equatable, Sendable {
    public var inputTokens: UInt64
    public var outputTokens: UInt64
    public var cacheCreationInputTokens: UInt64
    public var cacheReadInputTokens: UInt64

    public init(
        inputTokens: UInt64 = 0,
        outputTokens: UInt64 = 0,
        cacheCreationInputTokens: UInt64 = 0,
        cacheReadInputTokens: UInt64 = 0
    ) {
        self.inputTokens = inputTokens
        self.outputTokens = outputTokens
        self.cacheCreationInputTokens = cacheCreationInputTokens
        self.cacheReadInputTokens = cacheReadInputTokens
    }

    private enum CodingKeys: String, CodingKey {
        case inputTokens = "input_tokens"
        case outputTokens = "output_tokens"
        case cacheCreationInputTokens = "cache_creation_input_tokens"
        case cacheReadInputTokens = "cache_read_input_tokens"
    }
}

public struct UsageRecord: Equatable, Sendable {
    public var timestamp: Date
    public var usage: UsageData
    public var messageID: String

    public init(timestamp: Date, usage: UsageData, messageID: String) {
        self.timestamp = timestamp
        self.usage = usage
        self.messageID = messageID
    }
}

public enum SessionParser {
    public static func parseSessionFile(at url: URL) -> [UsageRecord] {
        guard let content = try? String(contentsOf: url, encoding: .utf8) else {
            return []
        }

        let decoder = JSONDecoder()
        let records = content.split(whereSeparator: \.isNewline).compactMap { rawLine -> UsageRecord? in
            guard !rawLine.isEmpty else {
                return nil
            }

            guard let data = rawLine.data(using: .utf8),
                  let entry = try? decoder.decode(JournalEntry.self, from: data),
                  entry.type == "assistant",
                  let message = entry.message,
                  message.role == "assistant",
                  let usage = message.usage,
                  let messageID = message.id,
                  let timestampString = entry.timestamp,
                  let timestamp = parseTimestamp(timestampString)
            else {
                return nil
            }

            return UsageRecord(timestamp: timestamp, usage: usage, messageID: messageID)
        }

        return deduplicate(records)
    }

    public static func loadAllSessions(rootDirectory: URL) -> [UsageRecord] {
        guard let enumerator = FileManager.default.enumerator(
            at: rootDirectory,
            includingPropertiesForKeys: [.isRegularFileKey],
            options: [.skipsHiddenFiles]
        ) else {
            return []
        }

        var allRecords: [UsageRecord] = []
        for case let fileURL as URL in enumerator {
            guard fileURL.pathExtension == "jsonl" else {
                continue
            }
            allRecords.append(contentsOf: parseSessionFile(at: fileURL))
        }
        return allRecords
    }

    public static func claudeProjectsDirectory(homeDirectory: URL? = FileManager.default.homeDirectoryForCurrentUser) -> URL {
        homeDirectory?
            .appendingPathComponent(".claude", isDirectory: true)
            .appendingPathComponent("projects", isDirectory: true)
        ?? URL(fileURLWithPath: NSHomeDirectory())
            .appendingPathComponent(".claude", isDirectory: true)
            .appendingPathComponent("projects", isDirectory: true)
    }

    private static func deduplicate(_ records: [UsageRecord]) -> [UsageRecord] {
        var bestByID: [String: UsageRecord] = [:]
        for record in records {
            guard let existing = bestByID[record.messageID] else {
                bestByID[record.messageID] = record
                continue
            }
            if record.usage.outputTokens > existing.usage.outputTokens {
                bestByID[record.messageID] = record
            }
        }
        return Array(bestByID.values)
    }

    private static func parseTimestamp(_ value: String) -> Date? {
        let isoFormatterWithFractional = ISO8601DateFormatter()
        isoFormatterWithFractional.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        if let date = isoFormatterWithFractional.date(from: value) {
            return date
        }
        let isoFormatter = ISO8601DateFormatter()
        isoFormatter.formatOptions = [.withInternetDateTime]
        return isoFormatter.date(from: value)
    }
}
