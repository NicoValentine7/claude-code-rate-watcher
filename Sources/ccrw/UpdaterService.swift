import CCRWCore
import Foundation

#if canImport(Sparkle)
import Sparkle

final class UpdaterService: NSObject {
    var onStateChange: ((UpdateState) -> Void)?
    private let currentVersion: String
    private let updater = SUUpdater.shared()

    init(currentVersion: String) {
        self.currentVersion = currentVersion
        super.init()
        updater?.delegate = self
    }

    @MainActor
    func start() {
        onStateChange?(UpdateState(phase: .idle, currentVersion: currentVersion))
    }

    @MainActor
    func checkForUpdates() {
        onStateChange?(UpdateState(phase: .checking, currentVersion: currentVersion))
        updater?.checkForUpdateInformation()
    }

    @MainActor
    func installUpdate() {
        updater?.checkForUpdates(nil)
    }
}

extension UpdaterService: SUUpdaterDelegate {
    func updater(_ updater: SUUpdater, didFindValidUpdate item: SUAppcastItem) {
        let callback = onStateChange
        let version = item.displayVersionString
        let description = item.itemDescription
        callback?(UpdateState(
            phase: .available,
            currentVersion: currentVersion,
            availableVersion: version,
            message: description
        ))
    }

    func updaterDidNotFindUpdate(_ updater: SUUpdater) {
        onStateChange?(UpdateState(
            phase: .upToDate,
            currentVersion: currentVersion
        ))
    }

    func updater(_ updater: SUUpdater, didAbortWithError error: Error) {
        onStateChange?(UpdateState(
            phase: .failed,
            currentVersion: currentVersion,
            message: error.localizedDescription
        ))
    }
}
#else
final class UpdaterService {
    var onStateChange: ((UpdateState) -> Void)?
    private let currentVersion: String

    init(currentVersion: String) {
        self.currentVersion = currentVersion
    }

    @MainActor
    func start() {
        onStateChange?(UpdateState(
            phase: .unsupported,
            currentVersion: currentVersion,
            message: "Sparkle is unavailable in this build."
        ))
    }

    @MainActor
    func checkForUpdates() {
        onStateChange?(UpdateState(
            phase: .unsupported,
            currentVersion: currentVersion,
            message: "Sparkle is unavailable in this build."
        ))
    }

    @MainActor
    func installUpdate() {
        onStateChange?(UpdateState(
            phase: .unsupported,
            currentVersion: currentVersion,
            message: "Sparkle is unavailable in this build."
        ))
    }
}
#endif
