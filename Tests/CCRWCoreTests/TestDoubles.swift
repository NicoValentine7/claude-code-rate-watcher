import CCRWCore
import Foundation

final class InMemoryKeychain: KeychainAccessing, @unchecked Sendable {
    private var storage: [String: String] = [:]

    func readGenericPassword(service: String?, account: String?) -> String? {
        storage[key(service: service, account: account)]
    }

    func upsertGenericPassword(_ value: String, service: String, account: String) throws {
        storage[key(service: service, account: account)] = value
    }

    func seed(_ value: String, service: String?, account: String?) {
        storage[key(service: service, account: account)] = value
    }

    private func key(service: String?, account: String?) -> String {
        "\(service ?? "_")::\(account ?? "_")"
    }
}

final class MockHTTPSession: HTTPSession, @unchecked Sendable {
    let handler: @Sendable (URLRequest) async throws -> (Data, HTTPURLResponse)

    init(handler: @escaping @Sendable (URLRequest) async throws -> (Data, HTTPURLResponse)) {
        self.handler = handler
    }

    func perform(_ request: URLRequest) async throws -> (Data, HTTPURLResponse) {
        try await handler(request)
    }
}

final class MockAuthProvider: AuthProviding, @unchecked Sendable {
    var credentialValue: AuthCredential?
    var expired = false
    var refreshTokenValue = "refreshed-token"
    var refreshError: Error?

    init(credentialValue: AuthCredential?) {
        self.credentialValue = credentialValue
    }

    func credential() -> AuthCredential? {
        credentialValue
    }

    func isTokenExpired(now: Date) -> Bool {
        expired
    }

    func refreshToken() async throws -> String {
        if let refreshError {
            throw refreshError
        }
        credentialValue = .bearer(refreshTokenValue)
        return refreshTokenValue
    }
}

final class MockClaudeCLI: ClaudeCLIControlling, @unchecked Sendable {
    var authStatusValue = false
    var authLoginError: Error?

    func authStatus() async -> Bool {
        authStatusValue
    }

    func authLogin() async throws {
        if let authLoginError {
            throw authLoginError
        }
    }
}

func httpResponse(
    url: URL,
    statusCode: Int,
    headers: [String: String] = [:]
) -> HTTPURLResponse {
    HTTPURLResponse(
        url: url,
        statusCode: statusCode,
        httpVersion: nil,
        headerFields: headers
    )!
}
