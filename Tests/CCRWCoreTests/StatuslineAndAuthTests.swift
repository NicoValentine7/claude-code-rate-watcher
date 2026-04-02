import CCRWCore
import Foundation
import Testing

struct StatuslineAndAuthTests {
    @Test
    func statuslineReaderRejectsStaleDataAndParsesEpochResets() throws {
        let home = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        let claudeDirectory = home.appendingPathComponent(".claude", isDirectory: true)
        try FileManager.default.createDirectory(at: claudeDirectory, withIntermediateDirectories: true)
        let fileURL = claudeDirectory.appendingPathComponent(StatuslineService.rateDataFilename)

        let now = Date(timeIntervalSince1970: 1_775_200_000)
        let freshPayload = """
        {"five_hour_percent":64,"seven_day_percent":22,"five_hour_resets_at":"1775203600","seven_day_resets_at":"2026-04-04T03:00:00Z","timestamp":1775200000}
        """
        try freshPayload.write(to: fileURL, atomically: true, encoding: .utf8)

        let service = StatuslineService(homeDirectory: home)
        let snapshot = try #require(service.readRateData(now: now))
        #expect(snapshot.fiveHourPercent == 64)
        #expect(snapshot.fiveHourResetsAt == Date(timeIntervalSince1970: 1_775_203_600))

        let stalePayload = """
        {"five_hour_percent":64,"timestamp":1775190000}
        """
        try stalePayload.write(to: fileURL, atomically: true, encoding: .utf8)
        #expect(service.readRateData(now: now) == nil)
    }

    @Test
    func authStoreDetectsExpiryAndRefreshesCredentials() async throws {
        let keychain = InMemoryKeychain()
        keychain.seed("""
        {"claudeAiOauth":{"accessToken":"old-access","refreshToken":"refresh-me","expiresAt":1775200100000}}
        """, service: AuthStore.credentialService, account: AuthStore.credentialService)

        #expect(AuthStore.isTokenExpired(
            rawCredential: #"{"claudeAiOauth":{"expiresAt":1775199700000}}"#,
            now: Date(timeIntervalSince1970: 1_775_200_000)
        ))

        let session = MockHTTPSession { request in
            #expect(request.url == AuthStore.oauthTokenURL)
            let data = """
            {"access_token":"new-access","refresh_token":"new-refresh","expires_in":3600}
            """.data(using: .utf8)!
            return (data, httpResponse(url: AuthStore.oauthTokenURL, statusCode: 200))
        }

        let store = AuthStore(session: session, keychain: keychain)
        let refreshed = try await store.refreshToken()

        #expect(refreshed == "new-access")
        let persisted = try #require(keychain.readGenericPassword(service: AuthStore.credentialService, account: AuthStore.credentialService))
        #expect(persisted.contains("new-access"))
        #expect(persisted.contains("new-refresh"))
    }
}
