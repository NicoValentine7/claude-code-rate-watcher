import AppKit

@MainActor
final class StatusBarController {
    private let statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
    private let debugLabel = ProcessInfo.processInfo.environment["CCRW_DEBUG_LABEL"]

    var button: NSStatusBarButton? {
        statusItem.button
    }

    var onToggle: (() -> Void)?

    init() {
        guard let button = statusItem.button else {
            return
        }

        button.target = self
        button.action = #selector(togglePopover)
        button.imagePosition = .imageLeading
        button.font = NSFont.monospacedDigitSystemFont(ofSize: 12, weight: .semibold)
        update(percent: 0, title: formattedTitle(for: 0))
        statusItem.menu = nil
    }

    func update(percent: Int, title: String) {
        statusItem.button?.image = StatusIconFactory.image(for: percent)
        statusItem.button?.title = title
        statusItem.button?.toolTip = BuildInfo.bundleName
    }

    func formattedTitle(for percent: Int) -> String {
        if let debugLabel, !debugLabel.isEmpty {
            return "[\(debugLabel)] \(percent)%"
        }
        return "\(percent)%"
    }

    @objc
    private func togglePopover() {
        onToggle?()
    }
}
