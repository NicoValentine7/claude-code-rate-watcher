import AppKit
import CCRWCore
import Foundation

@MainActor
final class AppState: ObservableObject {
    @Published private(set) var usageSummary: UsageSummary = .empty
    @Published private(set) var snapshot: RateLimitSnapshot = .empty
    @Published private(set) var authState = AuthState()
    @Published private(set) var updateState = UpdateState(currentVersion: BuildInfo.currentVersion)
    @Published private(set) var diagnostics = AppDiagnostics()
    @Published private(set) var launchAtLoginEnabled = false
    @Published private(set) var statusBarPercent: Int = 0
    @Published private(set) var statusBarTitle: String = "0%"
    @Published private(set) var now = Date()
    @Published private(set) var isRefreshing = false
    @Published private(set) var isPopoverPresented = false

    private let statuslineService = StatuslineService()
    private let authStore = AuthStore()
    private let claudeCLI = ClaudeCLI()
    private let notificationService = NotificationService()
    private let launchAtLoginService = LaunchAtLoginService()
    private lazy var rateLimitService = RateLimitService(
        authProvider: authStore,
        claudeCLI: claudeCLI
    )
    private lazy var updater = UpdaterService(currentVersion: BuildInfo.currentVersion)
    private let sessionDebouncer = Debouncer(delay: 1.0)
    private let usageQueue = DispatchQueue(label: "ccrw.local-usage", qos: .utility)

    private var watcher: FileWatcherService?
    private var refreshLoop: Task<Void, Never>?
    private var cooldownLoop: Task<Void, Never>?
    private var hasStarted = false

    func start() {
        guard !hasStarted else {
            return
        }
        hasStarted = true

        statusBarTitle = formattedStatusTitle(for: 0)
        launchAtLoginEnabled = launchAtLoginService.isEnabled
        notificationService.requestAuthorization()
        configureUpdater()
        installStatuslineIfNeeded()
        startWatcher()
        startLoops()
        reloadLocalUsage()
        Task { await refreshLive(reason: "startup") }
    }

    func setPopoverPresented(_ presented: Bool) {
        isPopoverPresented = presented
        Task {
            await rateLimitService.setActive(presented)
            if presented {
                await refreshLive(reason: "popover-open")
            }
        }
    }

    func manualRefresh() {
        Task {
            await refreshLive(reason: "manual", manual: true)
        }
    }

    func reconnectClaude() {
        authState.isBusy = true
        authState.lastError = nil
        Task {
            do {
                try await claudeCLI.authLogin()
                authState.isBusy = false
                await refreshLive(reason: "auth-login")
            } catch {
                authState.isBusy = false
                authState.lastError = error.localizedDescription
                authState.status = .actionRequired
                diagnostics.lastError = error.localizedDescription
            }
        }
    }

    func openUsagePage() {
        guard let url = URL(string: "https://claude.ai/settings/usage") else {
            return
        }
        NSWorkspace.shared.open(url)
    }

    func setLaunchAtLogin(_ enabled: Bool) {
        do {
            try launchAtLoginService.setEnabled(enabled)
            launchAtLoginEnabled = enabled
        } catch {
            diagnostics.lastError = error.localizedDescription
        }
    }

    func checkForUpdates() {
        updater.checkForUpdates()
    }

    func installUpdate() {
        updater.installUpdate()
    }

    var effectiveFiveHourPercent: Int {
        usageSummary.effectiveFiveHourPercent(using: snapshot)
    }

    var effectiveSevenDayPercent: Int {
        usageSummary.effectiveSevenDayPercent(using: snapshot)
    }

    private func configureUpdater() {
        updater.onStateChange = { [weak self] state in
            Task { @MainActor [weak self] in
                self?.updateState = state
            }
        }
        updater.start()
    }

    private func installStatuslineIfNeeded() {
        do {
            if !statuslineService.isInstalled() {
                _ = try statuslineService.install()
            }
            diagnostics.statuslineInstalled = statuslineService.isInstalled()
        } catch {
            diagnostics.lastError = error.localizedDescription
        }
    }

    private func startWatcher() {
        let home = FileManager.default.homeDirectoryForCurrentUser
        let paths = [
            home.appendingPathComponent(".claude", isDirectory: true).appendingPathComponent("projects", isDirectory: true),
            home.appendingPathComponent(".claude", isDirectory: true),
        ]

        let watcher = FileWatcherService(paths: paths) { [weak self] event in
            Task { @MainActor [weak self] in
                guard let self else { return }
                switch event {
                case .sessionFilesChanged:
                    self.sessionDebouncer.schedule { [weak self] in
                        Task { @MainActor [weak self] in
                            self?.reloadLocalUsage()
                            await self?.refreshLive(reason: "session-update")
                        }
                    }
                case .statuslineChanged:
                    await self.refreshLive(reason: "statusline")
                }
            }
        }
        watcher.start()
        self.watcher = watcher
    }

    private func startLoops() {
        refreshLoop = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(30))
                await self?.tickRefresh()
            }
        }

        cooldownLoop = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(1))
                await self?.tickClock()
            }
        }
    }

    private func tickRefresh() async {
        now = Date()
        await refreshLive(reason: "timer")
    }

    private func tickClock() async {
        now = Date()
        let remaining = await rateLimitService.cooldownRemaining(now: now) ?? 0
        diagnostics.manualRefreshCooldownRemaining = remaining
        if remaining == 0, diagnostics.lastError?.contains("Refresh available in") == true {
            diagnostics.lastError = nil
        }
    }

    private func reloadLocalUsage() {
        let home = FileManager.default.homeDirectoryForCurrentUser
        usageQueue.async { [weak self] in
            let loaded = UsageCalculator.loadSummary(homeDirectory: home, now: Date())
            Task { @MainActor [weak self] in
                guard let self else { return }
                self.usageSummary = loaded.summary
                self.diagnostics.sessionRecordCount = loaded.records.count
                self.recomputePresentation()
            }
        }
    }

    private func refreshLive(reason: String, manual: Bool = false) async {
        isRefreshing = true
        let refreshNow = Date()
        now = refreshNow

        if manual {
            let executed = await rateLimitService.forcePoll(now: refreshNow)
            if !executed {
                let remaining = await rateLimitService.cooldownRemaining(now: refreshNow) ?? 0
                diagnostics.manualRefreshCooldownRemaining = remaining
                diagnostics.lastError = "Refresh available in \(remaining)s."
                isRefreshing = false
                return
            }
        } else {
            if let statuslineSnapshot = statuslineService.readRateData(now: refreshNow) {
                await rateLimitService.setStatuslineData(statuslineSnapshot)
            } else {
                await rateLimitService.clearStatusline()
            }
            await rateLimitService.poll(now: refreshNow)
        }

        let nextSnapshot = await rateLimitService.snapshot()
        let statuslineFresh = await rateLimitService.isUsingFreshStatusline()
        diagnostics.lastRefresh = refreshNow
        diagnostics.statuslineInstalled = statuslineService.isInstalled()
        diagnostics.statuslineFresh = statuslineFresh
        diagnostics.sourceDescription = sourceLabel(for: nextSnapshot)
        diagnostics.manualRefreshCooldownRemaining = await rateLimitService.cooldownRemaining(now: refreshNow) ?? 0
        diagnostics.lastError = nextSnapshot.errorMessage ?? nextSnapshot.errorDetail

        snapshot = nextSnapshot
        updateAuthState(from: nextSnapshot)
        recomputePresentation()
        notificationService.checkAndNotify(usagePercent: effectiveFiveHourPercent, now: refreshNow)
        isRefreshing = false
        _ = reason
    }

    private func updateAuthState(from snapshot: RateLimitSnapshot) {
        if authState.isBusy {
            authState.status = .refreshing
            authState.detail = "Waiting for Claude login…"
            return
        }

        if snapshot.authMissing {
            authState.status = .missing
            authState.detail = snapshot.errorMessage ?? "No Claude credentials found."
            return
        }

        if snapshot.suggestRelogin {
            authState.status = .actionRequired
            authState.detail = snapshot.errorMessage ?? "Claude session needs attention."
            authState.lastError = snapshot.errorDetail
            return
        }

        authState.status = .authenticated
        authState.detail = sourceLabel(for: snapshot)
        authState.lastError = snapshot.errorDetail
    }

    private func recomputePresentation() {
        statusBarPercent = effectiveFiveHourPercent
        statusBarTitle = formattedStatusTitle(for: effectiveFiveHourPercent)
    }

    private func formattedStatusTitle(for percent: Int) -> String {
        if let debugLabel = ProcessInfo.processInfo.environment["CCRW_DEBUG_LABEL"], !debugLabel.isEmpty {
            return "[\(debugLabel)] \(percent)%"
        }
        return "\(percent)%"
    }

    private func sourceLabel(for snapshot: RateLimitSnapshot) -> String {
        switch snapshot.source {
        case .liveAPI:
            return "Live API"
        case .statusline:
            return "Claude statusline"
        case .staleFallback:
            return "Stale API snapshot"
        case .localEstimate:
            return "Local estimate"
        case .unavailable:
            return "Unavailable"
        }
    }
}
