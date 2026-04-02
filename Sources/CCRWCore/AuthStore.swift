import Foundation
import Security

public enum AuthCredential: Equatable, Sendable {
    case cookie(String)
    case bearer(String)
}

public protocol HTTPSession: Sendable {
    func perform(_ request: URLRequest) async throws -> (Data, HTTPURLResponse)
}

extension URLSession: HTTPSession {
    public func perform(_ request: URLRequest) async throws -> (Data, HTTPURLResponse) {
        let (data, response) = try await data(for: request, delegate: nil)
        guard let httpResponse = response as? HTTPURLResponse else {
            throw AuthStoreError.invalidResponse("Non-HTTP response")
        }
        return (data, httpResponse)
    }
}

public protocol KeychainAccessing: Sendable {
    func readGenericPassword(service: String?, account: String?) -> String?
    func upsertGenericPassword(_ value: String, service: String, account: String) throws
}

public protocol AuthProviding: Sendable {
    func credential() -> AuthCredential?
    func isTokenExpired(now: Date) -> Bool
    func refreshToken() async throws -> String
}

public enum AuthStoreError: Error, LocalizedError, Sendable {
    case missingCredential(String)
    case invalidResponse(String)
    case refreshFailed(String)
    case keychainSaveFailed(String)

    public var errorDescription: String? {
        switch self {
        case let .missingCredential(message),
             let .invalidResponse(message),
             let .refreshFailed(message),
             let .keychainSaveFailed(message):
            return message
        }
    }
}

public final class SystemKeychain: KeychainAccessing, @unchecked Sendable {
    public init() {}

    public func readGenericPassword(service: String?, account: String?) -> String? {
        var query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne,
        ]
        if let service {
            query[kSecAttrService as String] = service
        }
        if let account {
            query[kSecAttrAccount as String] = account
        }

        var result: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        guard status == errSecSuccess,
              let data = result as? Data
        else {
            return nil
        }
        return String(data: data, encoding: .utf8)?
            .trimmingCharacters(in: .whitespacesAndNewlines)
    }

    public func upsertGenericPassword(_ value: String, service: String, account: String) throws {
        let deleteQuery: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
        ]
        SecItemDelete(deleteQuery as CFDictionary)

        let addQuery: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
            kSecValueData as String: Data(value.utf8),
        ]

        let status = SecItemAdd(addQuery as CFDictionary, nil)
        guard status == errSecSuccess else {
            throw AuthStoreError.keychainSaveFailed("Keychain save failed with status \(status)")
        }
    }
}

public final class AuthStore: AuthProviding, @unchecked Sendable {
    public static let oauthTokenURL = URL(string: "https://console.anthropic.com/v1/oauth/token")!
    public static let oauthClientID = "9d1c250a-e61b-44d9-88ed-5944d1962f5e"
    public static let credentialService = "Claude Code-credentials"
    public static let cookieAccount = "cookie.claude"

    private let session: any HTTPSession
    private let keychain: any KeychainAccessing

    public init(
        session: any HTTPSession = URLSession.shared,
        keychain: any KeychainAccessing = SystemKeychain()
    ) {
        self.session = session
        self.keychain = keychain
    }

    public func credential() -> AuthCredential? {
        if let token = bearerToken() {
            return .bearer(token)
        }
        if let cookie = cookieHeader() {
            return .cookie(cookie)
        }
        return nil
    }

    public func isTokenExpired(now: Date = Date()) -> Bool {
        guard let raw = readCredentialEntry() else {
            return true
        }
        return Self.isTokenExpired(rawCredential: raw, now: now)
    }

    public func refreshToken() async throws -> String {
        guard let raw = readCredentialEntry() else {
            throw AuthStoreError.missingCredential("No refresh token found")
        }
        var payload = try Self.parseJSONObject(from: raw)
        guard let claudeOAuth = payload["claudeAiOauth"] as? [String: Any],
              let refreshToken = claudeOAuth["refreshToken"] as? String
        else {
            throw AuthStoreError.missingCredential("No refresh token found")
        }

        var request = URLRequest(url: Self.oauthTokenURL)
        request.httpMethod = "POST"
        request.timeoutInterval = 15
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = try JSONSerialization.data(withJSONObject: [
            "grant_type": "refresh_token",
            "refresh_token": refreshToken,
            "client_id": Self.oauthClientID,
        ])

        let (data, response) = try await session.perform(request)
        guard (200 ..< 300).contains(response.statusCode) else {
            throw AuthStoreError.refreshFailed("Refresh request failed with HTTP \(response.statusCode)")
        }

        guard let refreshPayload = try? Self.parseJSONObject(from: data),
              let accessToken = refreshPayload["access_token"] as? String
        else {
            throw AuthStoreError.invalidResponse("No access_token in refresh response")
        }

        var updatedOAuth = claudeOAuth
        updatedOAuth["accessToken"] = accessToken
        if let newRefreshToken = refreshPayload["refresh_token"] as? String {
            updatedOAuth["refreshToken"] = newRefreshToken
        }
        if let expiresIn = refreshPayload["expires_in"] as? Double {
            updatedOAuth["expiresAt"] = Int64((Date().timeIntervalSince1970 + expiresIn) * 1000)
        } else if let expiresIn = refreshPayload["expires_in"] as? Int {
            updatedOAuth["expiresAt"] = Int64((Date().timeIntervalSince1970 + Double(expiresIn)) * 1000)
        }
        payload["claudeAiOauth"] = updatedOAuth

        let storedData = try JSONSerialization.data(withJSONObject: payload, options: [.sortedKeys])
        guard let storedString = String(data: storedData, encoding: .utf8) else {
            throw AuthStoreError.invalidResponse("Unable to serialize updated credential")
        }
        try keychain.upsertGenericPassword(
            storedString,
            service: Self.credentialService,
            account: Self.credentialService
        )
        return accessToken
    }

    public static func isTokenExpired(rawCredential: String, now: Date = Date()) -> Bool {
        guard let payload = try? parseJSONObject(from: rawCredential),
              let oauth = payload["claudeAiOauth"] as? [String: Any],
              let expiresAt = oauth["expiresAt"] as? NSNumber
        else {
            return true
        }

        let expiry = expiresAt.doubleValue / 1000.0
        return expiry < now.addingTimeInterval(300).timeIntervalSince1970
    }

    private func bearerToken() -> String? {
        guard let raw = readCredentialEntry(),
              let payload = try? Self.parseJSONObject(from: raw),
              let oauth = payload["claudeAiOauth"] as? [String: Any],
              let accessToken = oauth["accessToken"] as? String
        else {
            return nil
        }
        return accessToken
    }

    private func cookieHeader() -> String? {
        guard let raw = keychain.readGenericPassword(service: nil, account: Self.cookieAccount),
              let payload = try? Self.parseJSONObject(from: raw),
              let cookieHeader = payload["cookieHeader"] as? String
        else {
            return nil
        }
        return cookieHeader
    }

    private func readCredentialEntry() -> String? {
        keychain.readGenericPassword(service: Self.credentialService, account: nil)
            ?? keychain.readGenericPassword(service: Self.credentialService, account: Self.credentialService)
    }

    private static func parseJSONObject(from data: Data) throws -> [String: Any] {
        guard let payload = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            throw AuthStoreError.invalidResponse("Expected object payload")
        }
        return payload
    }

    private static func parseJSONObject(from string: String) throws -> [String: Any] {
        try parseJSONObject(from: Data(string.utf8))
    }
}
