import Foundation

private struct UsageResponse: Decodable {
    let fiveHour: UsageWindow?
    let sevenDay: UsageWindow?

    private enum CodingKeys: String, CodingKey {
        case fiveHour = "five_hour"
        case sevenDay = "seven_day"
    }
}

private struct UsageWindow: Decodable {
    let utilization: Double
    let resetsAt: String?

    private enum CodingKeys: String, CodingKey {
        case utilization
        case resetsAt = "resets_at"
    }
}

private struct FetchError: Error, Sendable {
    let message: String
    let detail: String
    let isAuthError: Bool
    let isRateLimited: Bool
}

private struct HTTPStatusError: Error, Sendable {
    let code: Int
    let body: String
}

public actor RateLimitService {
    public static let usageAPIURL = URL(string: "https://api.anthropic.com/api/oauth/usage")!
    public static let messagesAPIURL = URL(string: "https://api.anthropic.com/v1/messages")!
    public static let cacheTTLActive: TimeInterval = 90
    public static let cacheTTLIdle: TimeInterval = 300
    public static let manualRefreshCooldown: TimeInterval = 30
    public static let rateLimitBackoffs: [TimeInterval] = [15, 30, 60, 120, 300]

    private let authProvider: any AuthProviding
    private let session: any HTTPSession
    private let claudeCLI: any ClaudeCLIControlling

    private var current = RateLimitSnapshot.empty
    private var lastGood: RateLimitSnapshot?
    private var lastFetch: Date?
    private var cacheTTL = cacheTTLActive
    private var isActive = false
    private var statuslineFresh = false
    private var lastManualRefresh: Date?

    public init(
        authProvider: any AuthProviding = AuthStore(),
        session: any HTTPSession = URLSession.shared,
        claudeCLI: any ClaudeCLIControlling = ClaudeCLI()
    ) {
        self.authProvider = authProvider
        self.session = session
        self.claudeCLI = claudeCLI
    }

    public func setActive(_ active: Bool) {
        isActive = active
    }

    public func setStatuslineData(_ snapshot: RateLimitSnapshot) {
        statuslineFresh = true
        storeSuccess(snapshot)
    }

    public func clearStatusline() {
        statuslineFresh = false
    }

    public func isUsingFreshStatusline() -> Bool {
        statuslineFresh
    }

    public func snapshot() -> RateLimitSnapshot {
        if current.isLive {
            return current
        }
        if let lastGood {
            return current.staleFallback(using: lastGood)
        }
        return current
    }

    public func cooldownRemaining(now: Date = Date()) -> Int? {
        guard let lastManualRefresh else {
            return nil
        }
        let elapsed = now.timeIntervalSince(lastManualRefresh)
        guard elapsed < Self.manualRefreshCooldown else {
            return nil
        }
        return Int((Self.manualRefreshCooldown - elapsed).rounded(.up))
    }

    @discardableResult
    public func forcePoll(now: Date = Date()) async -> Bool {
        if let remaining = cooldownRemaining(now: now), remaining > 0 {
            return false
        }

        lastManualRefresh = now
        let priorStatuslineFresh = statuslineFresh
        statuslineFresh = false
        lastFetch = nil
        await poll(now: now)
        statuslineFresh = priorStatuslineFresh
        return true
    }

    public func poll(now: Date = Date()) async {
        if statuslineFresh {
            return
        }
        if let lastFetch, now.timeIntervalSince(lastFetch) < cacheTTL {
            return
        }

        guard let credential = authProvider.credential() else {
            current = RateLimitSnapshot(
                isLive: false,
                authMissing: true,
                errorMessage: "No credentials found",
                retryCount: current.retryCount,
                source: .unavailable
            )
            return
        }

        if case .bearer = credential, authProvider.isTokenExpired(now: now) {
            do {
                _ = try await authProvider.refreshToken()
            } catch {
                current = RateLimitSnapshot(
                    isLive: false,
                    authMissing: true,
                    errorMessage: "Token refresh failed",
                    errorDetail: error.localizedDescription,
                    retryCount: current.retryCount + 1,
                    source: .unavailable
                )
                return
            }
        }

        await tryFetch(credential: authProvider.credential() ?? credential, now: now)
    }

    public static func nextBackoff(for retryCount: Int) -> TimeInterval {
        let index = min(max(retryCount, 0), rateLimitBackoffs.count - 1)
        return rateLimitBackoffs[index]
    }

    private func storeSuccess(_ result: RateLimitSnapshot, now: Date = Date()) {
        lastGood = result
        current = result
        lastFetch = now
        cacheTTL = isActive ? Self.cacheTTLActive : Self.cacheTTLIdle
    }

    private func tryFetch(credential: AuthCredential, now: Date) async {
        do {
            let live = try await tryUsageAPI(credential: credential)
            storeSuccess(live, now: now)
            return
        } catch let error as FetchError {
            if error.isAuthError, case .bearer = credential {
                do {
                    _ = try await authProvider.refreshToken()
                    if let refreshed = authProvider.credential() {
                        let live = try await tryUsageAPI(credential: refreshed)
                        storeSuccess(live, now: now)
                        return
                    }
                } catch {
                    // fall through into other recovery paths
                }
            }

            if error.isRateLimited {
                await handleRateLimited(detail: error.detail, now: now)
                return
            }

            if case let .bearer(token) = credential {
                do {
                    let live = try await tryHaikuProbe(bearerToken: token)
                    storeSuccess(live, now: now)
                    return
                } catch let probeError as FetchError {
                    if probeError.isAuthError {
                        do {
                            _ = try await authProvider.refreshToken()
                            if case let .bearer(refreshedToken)? = authProvider.credential() {
                                let live = try await tryHaikuProbe(bearerToken: refreshedToken)
                                storeSuccess(live, now: now)
                                return
                            }
                        } catch {
                            current = RateLimitSnapshot(
                                isLive: false,
                                authMissing: true,
                                errorMessage: "Login required",
                                errorDetail: "Refresh failed: \(error.localizedDescription)",
                                retryCount: current.retryCount + 1,
                                source: .unavailable
                            )
                            return
                        }
                    }

                    current = RateLimitSnapshot(
                        isLive: false,
                        authMissing: false,
                        errorMessage: probeError.message,
                        errorDetail: probeError.detail,
                        retryCount: current.retryCount + 1,
                        source: .unavailable
                    )
                    lastFetch = now
                    return
                } catch {
                    current = RateLimitSnapshot(
                        isLive: false,
                        authMissing: false,
                        errorMessage: "Network error",
                        errorDetail: error.localizedDescription,
                        retryCount: current.retryCount + 1,
                        source: .unavailable
                    )
                    lastFetch = now
                    return
                }
            }

            current = RateLimitSnapshot(
                isLive: false,
                authMissing: false,
                errorMessage: "API unavailable",
                errorDetail: error.detail,
                retryCount: current.retryCount + 1,
                source: .unavailable
            )
            lastFetch = now
        } catch {
            current = RateLimitSnapshot(
                isLive: false,
                authMissing: false,
                errorMessage: "Network error",
                errorDetail: error.localizedDescription,
                retryCount: current.retryCount + 1,
                source: .unavailable
            )
            lastFetch = now
        }
    }

    private func handleRateLimited(detail: String, now: Date) async {
        let currentRetryCount = current.retryCount
        if currentRetryCount == 0, await claudeCLI.authStatus(), let refreshed = authProvider.credential() {
            if let live = try? await tryUsageAPI(credential: refreshed) {
                storeSuccess(live, now: now)
                return
            }
        }

        let backoff = Self.nextBackoff(for: currentRetryCount)
        current = RateLimitSnapshot(
            fiveHourPercent: current.fiveHourPercent,
            sevenDayPercent: current.sevenDayPercent,
            fiveHourResetsAt: current.fiveHourResetsAt,
            sevenDayResetsAt: current.sevenDayResetsAt,
            isLive: false,
            authMissing: false,
            errorMessage: "Rate limited (429)",
            errorDetail: detail,
            retryCount: currentRetryCount + 1,
            retryAt: now.addingTimeInterval(backoff),
            suggestRelogin: true,
            source: .unavailable
        )
        lastFetch = now
        cacheTTL = backoff
    }

    private func tryUsageAPI(credential: AuthCredential) async throws -> RateLimitSnapshot {
        var request = URLRequest(url: Self.usageAPIURL)
        request.timeoutInterval = 10
        request.httpMethod = "GET"
        request.setValue("application/json", forHTTPHeaderField: "Accept")
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("oauth-2025-04-20", forHTTPHeaderField: "anthropic-beta")
        request.setValue("claude-code-rate-watcher/1.0.0", forHTTPHeaderField: "User-Agent")
        applyCredential(credential, to: &request)

        let data: Data
        let response: HTTPURLResponse
        do {
            (data, response) = try await session.perform(request)
        } catch {
            throw FetchError(
                message: "Network error",
                detail: error.localizedDescription,
                isAuthError: false,
                isRateLimited: false
            )
        }

        guard (200 ..< 300).contains(response.statusCode) else {
            throw classify(statusCode: response.statusCode, data: data)
        }

        do {
            let body = try JSONDecoder().decode(UsageResponse.self, from: data)
            return RateLimitSnapshot(
                fiveHourPercent: body.fiveHour.map { Int($0.utilization.rounded()) },
                sevenDayPercent: body.sevenDay.map { Int($0.utilization.rounded()) },
                fiveHourResetsAt: parseDate(body.fiveHour?.resetsAt),
                sevenDayResetsAt: parseDate(body.sevenDay?.resetsAt),
                isLive: true,
                authMissing: false,
                errorMessage: nil,
                errorDetail: nil,
                retryCount: 0,
                retryAt: nil,
                suggestRelogin: false,
                source: .liveAPI
            )
        } catch {
            throw FetchError(
                message: "Invalid response",
                detail: "JSON parse error: \(error.localizedDescription)",
                isAuthError: false,
                isRateLimited: false
            )
        }
    }

    private func tryHaikuProbe(bearerToken: String) async throws -> RateLimitSnapshot {
        var request = URLRequest(url: Self.messagesAPIURL)
        request.timeoutInterval = 10
        request.httpMethod = "POST"
        request.setValue("Bearer \(bearerToken)", forHTTPHeaderField: "Authorization")
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("claude-code-rate-watcher/1.0.0", forHTTPHeaderField: "User-Agent")
        request.setValue("oauth-2025-04-20", forHTTPHeaderField: "anthropic-beta")
        request.setValue("2023-06-01", forHTTPHeaderField: "anthropic-version")
        request.httpBody = try JSONSerialization.data(withJSONObject: [
            "model": "claude-haiku-4-5-20251001",
            "max_tokens": 1,
            "messages": [["role": "user", "content": "h"]],
        ])

        let data: Data
        let response: HTTPURLResponse
        do {
            (data, response) = try await session.perform(request)
        } catch {
            throw FetchError(
                message: "Network error",
                detail: error.localizedDescription,
                isAuthError: false,
                isRateLimited: false
            )
        }

        guard (200 ..< 300).contains(response.statusCode) else {
            throw classify(statusCode: response.statusCode, data: data)
        }

        let fiveHourPercent = response.value(forHTTPHeaderField: "anthropic-ratelimit-unified-5h-utilization")
            .flatMap(Double.init)
            .map { Int(($0 * 100.0).rounded()) }
        let sevenDayPercent = response.value(forHTTPHeaderField: "anthropic-ratelimit-unified-7d-utilization")
            .flatMap(Double.init)
            .map { Int(($0 * 100.0).rounded()) }
        let fiveHourReset = parseDate(response.value(forHTTPHeaderField: "anthropic-ratelimit-unified-5h-reset"))
        let sevenDayReset = parseDate(response.value(forHTTPHeaderField: "anthropic-ratelimit-unified-7d-reset"))

        guard fiveHourPercent != nil || sevenDayPercent != nil else {
            throw FetchError(
                message: "No rate limit headers",
                detail: "Haiku probe succeeded but no unified rate limit headers found",
                isAuthError: false,
                isRateLimited: false
            )
        }

        return RateLimitSnapshot(
            fiveHourPercent: fiveHourPercent,
            sevenDayPercent: sevenDayPercent,
            fiveHourResetsAt: fiveHourReset,
            sevenDayResetsAt: sevenDayReset,
            isLive: true,
            authMissing: false,
            errorMessage: nil,
            errorDetail: nil,
            retryCount: 0,
            retryAt: nil,
            suggestRelogin: false,
            source: .liveAPI
        )
    }

    private func applyCredential(_ credential: AuthCredential, to request: inout URLRequest) {
        switch credential {
        case let .bearer(token):
            request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        case let .cookie(cookie):
            request.setValue(cookie, forHTTPHeaderField: "Cookie")
        }
    }

    private func classify(statusCode: Int, data: Data) -> FetchError {
        let body = String(data: data, encoding: .utf8) ?? "HTTP \(statusCode)"
        let isAuth = statusCode == 401 || statusCode == 403
        let isRate = statusCode == 429
        if isAuth {
            return FetchError(message: "Authentication error", detail: body, isAuthError: true, isRateLimited: false)
        }
        if isRate {
            return FetchError(message: "Rate limited (429)", detail: body, isAuthError: false, isRateLimited: true)
        }
        return FetchError(message: "HTTP \(statusCode)", detail: body, isAuthError: false, isRateLimited: false)
    }

    private func parseDate(_ raw: String?) -> Date? {
        guard let raw else {
            return nil
        }
        if let epoch = TimeInterval(raw) {
            return Date(timeIntervalSince1970: epoch)
        }
        if let withFractional = isoFormatterWithFractional.date(from: raw) {
            return withFractional
        }
        return isoFormatter.date(from: raw)
    }

    private var isoFormatterWithFractional: ISO8601DateFormatter {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return formatter
    }

    private var isoFormatter: ISO8601DateFormatter {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime]
        return formatter
    }
}
