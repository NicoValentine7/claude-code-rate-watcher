import Foundation

public protocol ClaudeCLIControlling: Sendable {
    func authStatus() async -> Bool
    func authLogin() async throws
}

public enum ClaudeCLIError: Error, LocalizedError, Sendable {
    case cliUnavailable
    case commandFailed(String)

    public var errorDescription: String? {
        switch self {
        case .cliUnavailable:
            return "Claude Code CLI not found. Install it first."
        case let .commandFailed(message):
            return message
        }
    }
}

public final class ClaudeCLI: ClaudeCLIControlling, @unchecked Sendable {
    private let fileManager: FileManager

    public init(fileManager: FileManager = .default) {
        self.fileManager = fileManager
    }

    public static func findBinary(fileManager: FileManager = .default) -> URL? {
        let home = fileManager.homeDirectoryForCurrentUser
        let candidates = [
            home.appendingPathComponent(".local/bin/claude"),
            home.appendingPathComponent(".claude/local/bin/claude"),
            URL(fileURLWithPath: "/opt/homebrew/bin/claude"),
            URL(fileURLWithPath: "/usr/local/bin/claude"),
        ]
        return candidates.first(where: { fileManager.isExecutableFile(atPath: $0.path) })
    }

    public func authStatus() async -> Bool {
        do {
            let result = try await run(arguments: ["auth", "status"])
            return result.exitCode == 0
        } catch {
            return false
        }
    }

    public func authLogin() async throws {
        let result = try await run(arguments: ["auth", "login"], captureOutput: true)
        guard result.exitCode == 0 else {
            let message = result.standardError
                .trimmingCharacters(in: .whitespacesAndNewlines)
            throw ClaudeCLIError.commandFailed(message.isEmpty ? "Claude login failed." : message)
        }
    }

    private func run(arguments: [String], captureOutput: Bool = false) async throws -> ProcessResult {
        let (executableURL, commandArguments) = command(arguments: arguments)

        return try await withCheckedThrowingContinuation { continuation in
            let process = Process()
            process.executableURL = executableURL
            process.arguments = commandArguments

            let stdout = Pipe()
            let stderr = Pipe()
            if captureOutput {
                process.standardOutput = stdout
                process.standardError = stderr
            } else {
                process.standardOutput = FileHandle.nullDevice
                process.standardError = FileHandle.nullDevice
            }

            process.terminationHandler = { process in
                let outData = captureOutput ? stdout.fileHandleForReading.readDataToEndOfFile() : Data()
                let errData = captureOutput ? stderr.fileHandleForReading.readDataToEndOfFile() : Data()
                continuation.resume(returning: ProcessResult(
                    exitCode: Int(process.terminationStatus),
                    standardOutput: String(data: outData, encoding: .utf8) ?? "",
                    standardError: String(data: errData, encoding: .utf8) ?? ""
                ))
            }

            do {
                try process.run()
            } catch {
                continuation.resume(throwing: ClaudeCLIError.cliUnavailable)
            }
        }
    }

    private func command(arguments: [String]) -> (URL, [String]) {
        if let executable = Self.findBinary(fileManager: fileManager) {
            return (executable, arguments)
        }
        return (URL(fileURLWithPath: "/usr/bin/env"), ["claude"] + arguments)
    }
}

private struct ProcessResult: Sendable {
    let exitCode: Int
    let standardOutput: String
    let standardError: String
}
