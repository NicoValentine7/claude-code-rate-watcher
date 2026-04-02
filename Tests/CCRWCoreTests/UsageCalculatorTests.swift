import CCRWCore
import Foundation
import Testing

struct UsageCalculatorTests {
    @Test
    func calculateUsageRespectsWindowsAndWeights() {
        let now = Date(timeIntervalSince1970: 1_775_200_000)
        let records = [
            UsageRecord(
                timestamp: now.addingTimeInterval(-2 * 3600),
                usage: UsageData(
                    inputTokens: 10_000_000,
                    outputTokens: 1_000_000,
                    cacheCreationInputTokens: 500_000,
                    cacheReadInputTokens: 1_000_000
                ),
                messageID: "recent"
            ),
            UsageRecord(
                timestamp: now.addingTimeInterval(-10 * 3600),
                usage: UsageData(
                    inputTokens: 20_000_000,
                    outputTokens: 2_000_000,
                    cacheCreationInputTokens: 0,
                    cacheReadInputTokens: 0
                ),
                messageID: "weekly"
            ),
        ]

        let summary = UsageCalculator.calculateUsage(records: records, now: now)

        #expect(summary.messageCount == 1)
        #expect(summary.usagePercent == 62)
        #expect(summary.weeklyMessageCount == 2)
        #expect(summary.weeklyUsagePercent == 20)
        #expect(CCRWFormatting.shortResetText(for: now.addingTimeInterval(65 * 60), now: now) == "Resets in: 1h 05m")
        #expect(CCRWFormatting.longResetText(for: now.addingTimeInterval((26 * 3600) + (5 * 60)), now: now) == "Resets in: 1d 2h 05m")
    }
}
