import CCRWCore
import SwiftUI

struct PopoverRootView: View {
    @EnvironmentObject private var state: AppState
    @State private var selectedTab: PopoverTab = .current

    private enum PopoverTab: String, CaseIterable {
        case current = "Current"
        case history = "History"
    }

    var body: some View {
        ZStack {
            background

            ScrollView(showsIndicators: false) {
                VStack(alignment: .leading, spacing: 14) {
                    header

                    Picker("View", selection: $selectedTab) {
                        ForEach(PopoverTab.allCases, id: \.self) { tab in
                            Text(tab.rawValue).tag(tab)
                        }
                    }
                    .pickerStyle(.segmented)
                    .padding(.horizontal, 2)

                    if selectedTab == .current {
                        heroCard
                        summaryStrip
                        if state.updateState.phase == .available {
                            updateBanner
                        }
                        if state.authState.status != .authenticated {
                            authCard
                        }
                        tokenBreakdown
                        diagnostics
                    } else {
                        UsageHistoryChart()
                    }
                }
                .padding(16)
            }
        }
        .frame(width: 388, height: 560)
    }

    private var background: some View {
        ZStack {
            LinearGradient(
                colors: [
                    Color(red: 0.15, green: 0.17, blue: 0.20),
                    Color(red: 0.10, green: 0.11, blue: 0.14),
                ],
                startPoint: .top,
                endPoint: .bottom
            )

            Circle()
                .fill(Color(red: 0.74, green: 0.79, blue: 0.84).opacity(0.10))
                .frame(width: 250, height: 250)
                .blur(radius: 28)
                .offset(x: 130, y: -220)

            Circle()
                .fill(Color(red: 0.82, green: 0.71, blue: 0.55).opacity(0.08))
                .frame(width: 220, height: 220)
                .blur(radius: 36)
                .offset(x: -150, y: 240)
        }
        .ignoresSafeArea()
    }

    private var header: some View {
        HStack(alignment: .top) {
            VStack(alignment: .leading, spacing: 5) {
                Text("Claude Code Rate Watcher")
                    .font(.system(size: 18, weight: .semibold, design: .rounded))
                    .foregroundStyle(.white.opacity(0.96))

                Text(state.diagnostics.sourceDescription)
                    .font(.system(size: 10, weight: .medium, design: .rounded))
                    .textCase(.uppercase)
                    .tracking(1.2)
                    .foregroundStyle(.white.opacity(0.46))
            }

            Spacer()

            VStack(alignment: .trailing, spacing: 6) {
                statusBadge
                Text("v\(state.updateState.currentVersion)")
                    .font(.system(size: 10, weight: .medium, design: .monospaced))
                    .foregroundStyle(.white.opacity(0.28))
            }
        }
        .padding(.horizontal, 2)
    }

    private var heroCard: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack(alignment: .top) {
                VStack(alignment: .leading, spacing: 6) {
                    capsuleLabel("5H WINDOW")
                    Text("\(state.effectiveFiveHourPercent)%")
                        .font(.system(size: 58, weight: .semibold, design: .rounded))
                        .tracking(-2.2)
                        .foregroundStyle(.white.opacity(0.98))
                    Text(heroResetText)
                        .font(.system(size: 13, weight: .medium, design: .rounded))
                        .foregroundStyle(.white.opacity(0.68))
                }

                Spacer()

                VStack(alignment: .trailing, spacing: 8) {
                    statBadge(
                        label: "7D",
                        value: "\(state.effectiveSevenDayPercent)%"
                    )
                    statBadge(
                        label: "MSGS",
                        value: "\(state.usageSummary.messageCount)"
                    )
                }
            }

            Text(heroSupportingCopy)
                .font(.system(size: 12, weight: .medium, design: .rounded))
                .foregroundStyle(.white.opacity(0.60))
                .fixedSize(horizontal: false, vertical: true)

            progressBar(for: state.effectiveFiveHourPercent)
                .frame(height: 8)
        }
        .padding(18)
        .background(calmPanel(fillOpacity: 0.075, strokeOpacity: 0.05))
    }

    private var summaryStrip: some View {
        HStack(spacing: 10) {
            summaryCard(
                title: "AUTH",
                value: authSummaryTitle,
                detail: authSummaryDetail
            )
            summaryCard(
                title: "REFRESH",
                value: refreshSummaryTitle,
                detail: refreshSummaryDetail
            )
        }
    }

    private var updateBanner: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(alignment: .center) {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Update ready")
                        .font(.system(size: 13, weight: .semibold, design: .rounded))
                        .foregroundStyle(.white.opacity(0.95))
                    Text("Version \(state.updateState.availableVersion ?? "new") is available.")
                        .font(.system(size: 12, weight: .medium, design: .rounded))
                        .foregroundStyle(.white.opacity(0.60))
                }
                Spacer()
                Button("Install") {
                    state.installUpdate()
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.small)
                .tint(.white.opacity(0.92))
                .foregroundStyle(Color.black.opacity(0.85))
            }
        }
        .padding(14)
        .background(calmPanel(fillOpacity: 0.06, strokeOpacity: 0.05))
    }

    private var authCard: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("Claude authentication needs attention")
                .font(.system(size: 13, weight: .semibold, design: .rounded))
                .foregroundStyle(.white.opacity(0.95))

            Text(state.authState.detail ?? "Reconnect the Claude CLI so live usage can resume.")
                .font(.system(size: 12, weight: .medium, design: .rounded))
                .foregroundStyle(.white.opacity(0.66))

            if let lastError = state.authState.lastError, !lastError.isEmpty {
                Text(lastError)
                    .font(.system(size: 11, weight: .medium, design: .monospaced))
                    .foregroundStyle(Color(red: 0.92, green: 0.72, blue: 0.60))
                    .textSelection(.enabled)
            }

            Button(state.authState.isBusy ? "Opening Claude…" : "Reconnect Claude") {
                state.reconnectClaude()
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.small)
            .tint(.white.opacity(0.92))
            .foregroundStyle(Color.black.opacity(0.85))
            .disabled(state.authState.isBusy)
        }
        .padding(14)
        .background(calmPanel(fillOpacity: 0.06, strokeOpacity: 0.05))
    }

    private var tokenBreakdown: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("Weighted token detail")
                .font(.system(size: 12, weight: .semibold, design: .rounded))
                .foregroundStyle(.white.opacity(0.82))

            VStack(spacing: 8) {
                detailRow(label: "Input", value: state.usageSummary.totalInputTokens)
                detailRow(label: "Output ×5", value: state.usageSummary.totalOutputTokens)
                detailRow(label: "Cache write", value: state.usageSummary.totalCacheCreationTokens)
                detailRow(label: "Cache read ÷10", value: state.usageSummary.totalCacheReadTokens)
            }
        }
        .padding(14)
        .background(calmPanel(fillOpacity: 0.048, strokeOpacity: 0.045))
    }

    private var diagnostics: some View {
        DisclosureGroup {
            VStack(alignment: .leading, spacing: 8) {
                diagnosticsRow("Last refresh", CCRWFormatting.compactTimestamp(state.diagnostics.lastRefresh))
                diagnosticsRow("Statusline", state.diagnostics.statuslineInstalled ? (state.diagnostics.statuslineFresh ? "Installed · Fresh" : "Installed · Idle") : "Not installed")
                diagnosticsRow("Records", state.diagnostics.sessionRecordCount.formatted())

                if let error = state.diagnostics.lastError, !error.isEmpty {
                    Text(error)
                        .font(.system(size: 11, weight: .medium, design: .monospaced))
                        .foregroundStyle(Color(red: 0.88, green: 0.73, blue: 0.54))
                        .textSelection(.enabled)
                        .padding(.top, 4)
                }
            }
            .padding(.top, 8)
        } label: {
            Text("Diagnostics")
                .font(.system(size: 12, weight: .semibold, design: .rounded))
                .foregroundStyle(.white.opacity(0.72))
        }
        .tint(.white.opacity(0.72))
        .padding(14)
        .background(calmPanel(fillOpacity: 0.038, strokeOpacity: 0.04))
    }

    private var heroResetText: String {
        state.snapshot.fiveHourResetsAt.map { CCRWFormatting.shortResetText(for: $0, now: state.now) }
            ?? state.usageSummary.resetTime.map { CCRWFormatting.shortResetText(for: $0, now: state.now) }
            ?? "Waiting for activity"
    }

    private var heroSupportingCopy: String {
        let percent = state.effectiveFiveHourPercent
        if percent >= 90 {
            return "Critical range. It is a good moment to pause and wait for the next reset."
        }
        if percent >= 70 {
            return "Approaching the warning line. Keep an eye on the next few requests."
        }
        return "Quiet monitoring mode. The main limit still has room before the warning threshold."
    }

    private var authSummaryTitle: String {
        switch state.authState.status {
        case .checking:
            return "Checking"
        case .authenticated:
            return "Connected"
        case .refreshing:
            return "Refreshing"
        case .missing:
            return "Missing"
        case .actionRequired:
            return "Needs attention"
        }
    }

    private var authSummaryDetail: String {
        state.authState.detail ?? "Claude credentials"
    }

    private var refreshSummaryTitle: String {
        if state.isRefreshing {
            return "Refreshing"
        }
        if state.diagnostics.manualRefreshCooldownRemaining > 0 {
            return "in \(state.diagnostics.manualRefreshCooldownRemaining)s"
        }
        return "Ready"
    }

    private var refreshSummaryDetail: String {
        state.updateState.phase == .available ? "update available" : "manual refresh"
    }

    private func summaryCard(title: String, value: String, detail: String) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            capsuleLabel(title)
            Text(value)
                .font(.system(size: 16, weight: .semibold, design: .rounded))
                .foregroundStyle(.white.opacity(0.92))
                .lineLimit(1)
                .minimumScaleFactor(0.85)
            Text(detail)
                .font(.system(size: 11, weight: .medium, design: .rounded))
                .foregroundStyle(.white.opacity(0.56))
                .lineLimit(2)
                .fixedSize(horizontal: false, vertical: true)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(14)
        .background(calmPanel(fillOpacity: 0.052, strokeOpacity: 0.045))
    }

    private func statBadge(label: String, value: String) -> some View {
        VStack(alignment: .trailing, spacing: 4) {
            Text(label)
                .font(.system(size: 9, weight: .semibold, design: .rounded))
                .textCase(.uppercase)
                .tracking(1.0)
                .foregroundStyle(.white.opacity(0.42))
            Text(value)
                .font(.system(size: 14, weight: .semibold, design: .monospaced))
                .foregroundStyle(.white.opacity(0.82))
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 8)
        .background(
            RoundedRectangle(cornerRadius: 14, style: .continuous)
                .fill(Color.white.opacity(0.045))
                .overlay(
                    RoundedRectangle(cornerRadius: 14, style: .continuous)
                        .stroke(Color.white.opacity(0.04), lineWidth: 1)
                )
        )
    }

    private func capsuleLabel(_ text: String) -> some View {
        Text(text)
            .font(.system(size: 10, weight: .semibold, design: .rounded))
            .textCase(.uppercase)
            .tracking(1.2)
            .foregroundStyle(.white.opacity(0.44))
    }

    private func progressBar(for percent: Int) -> some View {
        GeometryReader { proxy in
            ZStack(alignment: .leading) {
                Capsule()
                    .fill(Color.white.opacity(0.07))
                Capsule()
                    .fill(statusColor(for: percent).opacity(0.88))
                    .frame(width: max(20, proxy.size.width * CGFloat(percent) / 100.0))
            }
        }
    }

    private func calmPanel(fillOpacity: Double, strokeOpacity: Double) -> some View {
        RoundedRectangle(cornerRadius: 20, style: .continuous)
            .fill(Color.white.opacity(fillOpacity))
            .overlay(
                RoundedRectangle(cornerRadius: 20, style: .continuous)
                    .stroke(Color.white.opacity(strokeOpacity), lineWidth: 1)
            )
    }

    private func detailRow(label: String, value: UInt64) -> some View {
        HStack {
            Text(label)
                .font(.system(size: 12, weight: .medium, design: .rounded))
                .foregroundStyle(.white.opacity(0.56))
            Spacer()
            Text(value.formatted())
                .font(.system(size: 12, weight: .semibold, design: .monospaced))
                .foregroundStyle(.white.opacity(0.84))
        }
    }

    private func diagnosticsRow(_ label: String, _ value: String) -> some View {
        HStack {
            Text(label)
                .font(.system(size: 12, weight: .medium, design: .rounded))
                .foregroundStyle(.white.opacity(0.50))
            Spacer()
            Text(value)
                .font(.system(size: 11, weight: .semibold, design: .monospaced))
                .foregroundStyle(.white.opacity(0.80))
        }
    }

    private var statusBadge: some View {
        Text(state.snapshot.isLive ? "LIVE" : "FALLBACK")
            .font(.system(size: 10, weight: .bold, design: .rounded))
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            .background(
                Capsule()
                    .fill(
                        state.snapshot.isLive
                            ? Color(red: 0.42, green: 0.78, blue: 0.60).opacity(0.14)
                            : Color(red: 0.82, green: 0.71, blue: 0.55).opacity(0.14)
                    )
            )
            .foregroundStyle(
                state.snapshot.isLive
                    ? Color(red: 0.84, green: 0.92, blue: 0.86)
                    : Color(red: 0.92, green: 0.82, blue: 0.69)
            )
    }

    private func statusColor(for percent: Int) -> Color {
        if percent >= 90 {
            return Color(red: 0.83, green: 0.55, blue: 0.49)
        }
        if percent >= 70 {
            return Color(red: 0.82, green: 0.71, blue: 0.55)
        }
        return Color(red: 0.47, green: 0.77, blue: 0.60)
    }
}
