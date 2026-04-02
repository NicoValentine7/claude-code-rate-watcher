import AppKit
import Combine
import SwiftUI

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate, NSPopoverDelegate {
    private let state = AppState()
    private let statusBarController = StatusBarController()
    private let popover = NSPopover()
    private var cancellables: Set<AnyCancellable> = []

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.accessory)

        popover.behavior = .transient
        popover.animates = true
        popover.delegate = self
        popover.contentSize = NSSize(width: 388, height: 560)
        popover.contentViewController = NSHostingController(
            rootView: PopoverRootView().environmentObject(state)
        )

        statusBarController.onToggle = { [weak self] in
            self?.togglePopover()
        }

        state.$statusBarPercent
            .combineLatest(state.$statusBarTitle)
            .sink { [weak self] percent, title in
                self?.statusBarController.update(percent: percent, title: title)
            }
            .store(in: &cancellables)

        state.start()
    }

    func applicationWillTerminate(_ notification: Notification) {
        cancellables.removeAll()
    }

    func popoverDidClose(_ notification: Notification) {
        state.setPopoverPresented(false)
    }

    private func togglePopover() {
        guard let button = statusBarController.button else {
            return
        }

        if popover.isShown {
            popover.performClose(nil)
            state.setPopoverPresented(false)
        } else {
            popover.show(relativeTo: button.bounds, of: button, preferredEdge: .minY)
            state.setPopoverPresented(true)
            NSApp.activate(ignoringOtherApps: true)
        }
    }
}
