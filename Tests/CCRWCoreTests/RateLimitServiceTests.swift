import CCRWCore
import Foundation
import Testing

struct RateLimitServiceTests {
    @Test
    func pollUsesUsageAPIResponse() async {
        let auth = MockAuthProvider(credentialValue: .bearer("token-123"))
        let session = MockHTTPSession { request in
            #expect(request.url == RateLimitService.usageAPIURL)
            let body = """
            {"five_hour":{"utilization":61.0,"resets_at":"2026-04-03T10:00:00Z"},"seven_day":{"utilization":15.0,"resets_at":"2026-04-06T10:00:00Z"}}
            """.data(using: .utf8)!
            return (body, httpResponse(url: RateLimitService.usageAPIURL, statusCode: 200))
        }

        let service = RateLimitService(authProvider: auth, session: session, claudeCLI: MockClaudeCLI())
        await service.poll(now: Date(timeIntervalSince1970: 1_775_200_000))
        let snapshot = await service.snapshot()

        #expect(snapshot.fiveHourPercent == 61)
        #expect(snapshot.sevenDayPercent == 15)
        #expect(snapshot.source == .liveAPI)
        #expect(snapshot.isLive)
    }

    @Test
    func pollBacksOffOn429() async {
        let auth = MockAuthProvider(credentialValue: .bearer("token-123"))
        let session = MockHTTPSession { request in
            let body = Data("rate limited".utf8)
            return (body, httpResponse(url: request.url!, statusCode: 429))
        }
        let cli = MockClaudeCLI()
        cli.authStatusValue = false

        let service = RateLimitService(authProvider: auth, session: session, claudeCLI: cli)
        let now = Date(timeIntervalSince1970: 1_775_200_000)
        await service.poll(now: now)
        let snapshot = await service.snapshot()

        #expect(snapshot.errorMessage == "Rate limited (429)")
        #expect(snapshot.retryCount == 1)
        #expect(snapshot.retryAt == now.addingTimeInterval(15))
        #expect(snapshot.suggestRelogin)
        #expect(RateLimitService.nextBackoff(for: 3) == 120)
    }
}
