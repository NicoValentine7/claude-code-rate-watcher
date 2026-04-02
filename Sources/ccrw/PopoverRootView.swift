import CCRWCore
import SwiftUI

struct PopoverRootView: View {
    @EnvironmentObject private var state: AppState

    var body: some View {
        ZStack {
            LinearGradient(
                colors: [
                    Color(red: 0.12, green: 0.14, blue: 0.19),
                    Color(red: 0.08, green: 0.10, blue: 0.13),
                ],
                startPoint: .topLeading,
                endPoint: .bottomTrailing
            )

            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    header
                    if state.updateState.phase == .available {
                        updateBanner
                    }
                    if state.authState.status != .authenticated {
                        authCard
                    }
                    usageCard(
                        title: "5 Hour Window",
                        subtitle: state.snapshot.fiveHourPercent != nil ? "Live rate data" : "Local estimate fallback",
                        percent: state.effectiveFiveHourPercent,
                        resetText: state.snapshot.fiveHourResetsAt.map { CCRWFormatting.shortResetText(for: $0, now: state.now) }
                            ?? state.usageSummary.resetTime.map { CCRWFormatting.shortResetText(for: $0, now: state.now) }
                            ?? "Waiting for activity",
                        tokens: state.usageSummary.totalTokens,
                        messageCount: state.usageSummary.messageCount
                    )
                    usageCard(
                        title: "7 Day Window",
                        subtitle: state.snapshot.sevenDayPercent != nil ? "Live rate data" : "Local estimate fallback",
                        percent: state.effectiveSevenDayPercent,
                        resetText: state.snapshot.sevenDayResetsAt.map { CCRWFormatting.longResetText(for: $0, now: state.now) }
                            ?? state.usageSummary.weeklyResetTime.map { CCRWFormatting.longResetText(for: $0, now: state.now) }
                            ?? "Waiting for activity",
                        tokens: state.usageSummary.weeklyTotalTokens,
                        messageCount: state.usageSummary.weeklyMessageCount
                    )
                    tokenBreakdown
                    actions
                    diagnostics
                }
                .padding(18)
            }
        }
        .frame(width: 388, height: 560)
    }

    private var header: some View {
        HStack(alignment: .top) {
            VStack(alignment: .leading, spacing: 6) {
                Text("Claude Code Rate Watcher")
                    .font(.system(size: 20, weight: .bold, design: .rounded))
                    .foregroundStyle(.white)
                Text(state.diagnostics.sourceDescription)
                    .font(.system(size: 12, weight: .medium, design: .rounded))
                    .foregroundStyle(.white.opacity(0.62))
            }
            Spacer()
            VStack(alignment: .trailing, spacing: 6) {
                statusBadge
                Text("v\(state.updateState.currentVersion)")
                    .font(.system(size: 11, weight: .medium, design: .monospaced))
                    .foregroundStyle(.white.opacity(0.35))
            }
        }
    }

    private var statusBadge: some View {
        Text(state.snapshot.isLive ? "LIVE" : "FALLBACK")
            .font(.system(size: 11, weight: .black, design: .rounded))
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            .background(
                Capsule()
                    .fill(state.snapshot.isLive ? Color.green.opacity(0.18) : Color.orange.opacity(0.18))
            )
            .foregroundStyle(state.snapshot.isLive ? Color.green : Color.orange)
    }

    private var updateBanner: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("Update available")
                .font(.system(size: 13, weight: .bold, design: .rounded))
            Text("Version \(state.updateState.availableVersion ?? "new") is ready to install.")
                .font(.system(size: 12, weight: .medium, design: .rounded))
                .foregroundStyle(.white.opacity(0.75))
            Button("Install Update") {
                state.installUpdate()
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.small)
        }
        .padding(14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(
            RoundedRectangle(cornerRadius: 18, style: .continuous)
                .fill(Color.blue.opacity(0.18))
        )
    }

    private var authCard: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("Claude authentication needs attention")
                .font(.system(size: 14, weight: .bold, design: .rounded))
                .foregroundStyle(.white)
            Text(state.authState.detail ?? "Reconnect the Claude CLI so live usage can resume.")
                .font(.system(size: 12, weight: .medium, design: .rounded))
                .foregroundStyle(.white.opacity(0.75))
            if let lastError = state.authState.lastError, !lastError.isEmpty {
                Text(lastError)
                    .font(.system(size: 11, weight: .medium, design: .monospaced))
                    .foregroundStyle(Color.red.opacity(0.9))
                    .textSelection(.enabled)
            }
            Button(state.authState.isBusy ? "Opening Claude…" : "Reconnect Claude") {
                state.reconnectClaude()
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.small)
            .disabled(state.authState.isBusy)
        }
        .padding(14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(
            RoundedRectangle(cornerRadius: 18, style: .continuous)
                .fill(Color.red.opacity(0.16))
        )
    }

    private func usageCard(title: String, subtitle: String, percent: Int, resetText: String, tokens: UInt64, messageCount: Int) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(alignment: .top) {
                VStack(alignment: .leading, spacing: 4) {
                    Text(title)
                        .font(.system(size: 14, weight: .bold, design: .rounded))
                        .foregroundStyle(.white)
                    Text(subtitle)
                        .font(.system(size: 11, weight: .medium, design: .rounded))
                        .foregroundStyle(.white.opacity(0.55))
                }
                Spacer()
                Text("\(percent)%")
                    .font(.system(size: 30, weight: .black, design: .rounded))
                    .foregroundStyle(statusColor(for: percent))
            }

            GeometryReader { proxy in
                ZStack(alignment: .leading) {
                    Capsule()
                        .fill(Color.white.opacity(0.08))
                    Capsule()
                        .fill(statusColor(for: percent))
                        .frame(width: max(18, proxy.size.width * CGFloat(percent) / 100.0))
                }
            }
            .frame(height: 10)

            HStack {
                statPill(label: resetText)
                statPill(label: "\(messageCount) msgs")
                Spacer()
                Text(tokens.formatted())
                    .font(.system(size: 12, weight: .semibold, design: .monospaced))
                    .foregroundStyle(.white.opacity(0.82))
            }
        }
        .padding(16)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(
            RoundedRectangle(cornerRadius: 20, style: .continuous)
                .fill(Color.white.opacity(0.06))
                .overlay(
                    RoundedRectangle(cornerRadius: 20, style: .continuous)
                        .stroke(Color.white.opacity(0.06), lineWidth: 1)
                )
        )
    }

    private var tokenBreakdown: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("Weighted token detail")
                .font(.system(size: 13, weight: .bold, design: .rounded))
                .foregroundStyle(.white)
            Grid(alignment: .leading, horizontalSpacing: 10, verticalSpacing: 6) {
                breakdownRow(label: "Input", value: state.usageSummary.totalInputTokens)
                breakdownRow(label: "Output", value: state.usageSummary.totalOutputTokens)
                breakdownRow(label: "Cache write", value: state.usageSummary.totalCacheCreationTokens)
                breakdownRow(label: "Cache read", value: state.usageSummary.totalCacheReadTokens)
            }
        }
        .padding(14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(
            RoundedRectangle(cornerRadius: 18, style: .continuous)
                .fill(Color.black.opacity(0.16))
        )
    }

    private var actions: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 10) {
                Button(state.diagnostics.manualRefreshCooldownRemaining > 0 ? "Refresh in \(state.diagnostics.manualRefreshCooldownRemaining)s" : "Manual Refresh") {
                    state.manualRefresh()
                }
                .buttonStyle(.borderedProminent)
                .disabled(state.diagnostics.manualRefreshCooldownRemaining > 0 || state.isRefreshing)

                Button("Usage Page") {
                    state.openUsagePage()
                }
                .buttonStyle(.bordered)

                Button("Check Updates") {
                    state.checkForUpdates()
                }
                .buttonStyle(.bordered)
            }

            Toggle(isOn: Binding(
                get: { state.launchAtLoginEnabled },
                set: { state.setLaunchAtLogin($0) }
            )) {
                Text("Launch at login")
                    .font(.system(size: 13, weight: .semibold, design: .rounded))
                    .foregroundStyle(.white)
            }
            .toggleStyle(.switch)
            .tint(.green)
        }
        .padding(14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(
            RoundedRectangle(cornerRadius: 18, style: .continuous)
                .fill(Color.white.opacity(0.06))
        )
    }

    private var diagnostics: some View {
        DisclosureGroup("Diagnostics") {
            VStack(alignment: .leading, spacing: 8) {
                diagnosticRow("Last refresh", CCRWFormatting.compactTimestamp(state.diagnostics.lastRefresh))
                diagnosticRow("Statusline", state.diagnostics.statuslineInstalled ? (state.diagnostics.statuslineFresh ? "Installed · Fresh" : "Installed · Idle") : "Not installed")
                diagnosticRow("Records", state.diagnostics.sessionRecordCount.formatted())
                if let error = state.diagnostics.lastError, !error.isEmpty {
                    Text(error)
                        .font(.system(size: 11, weight: .medium, design: .monospaced))
                        .foregroundStyle(Color.orange.opacity(0.9))
                        .textSelection(.enabled)
                }
            }
            .padding(.top, 8)
        }
        .font(.system(size: 12, weight: .semibold, design: .rounded))
        .foregroundStyle(.white.opacity(0.8))
        .padding(14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(
            RoundedRectangle(cornerRadius: 18, style: .continuous)
                .fill(Color.white.opacity(0.04))
        )
    }

    private func statPill(label: String) -> some View {
        Text(label)
            .font(.system(size: 11, weight: .semibold, design: .rounded))
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            .background(
                Capsule()
                    .fill(Color.white.opacity(0.08))
            )
            .foregroundStyle(.white.opacity(0.72))
    }

    private func breakdownRow(label: String, value: UInt64) -> some View {
        GridRow {
            Text(label)
                .font(.system(size: 12, weight: .medium, design: .rounded))
                .foregroundStyle(.white.opacity(0.62))
            Text(value.formatted())
                .font(.system(size: 12, weight: .semibold, design: .monospaced))
                .foregroundStyle(.white.opacity(0.92))
        }
    }

    private func diagnosticRow(_ label: String, _ value: String) -> some View {
        HStack {
            Text(label)
                .foregroundStyle(.white.opacity(0.52))
            Spacer()
            Text(value)
                .font(.system(size: 11, weight: .semibold, design: .monospaced))
                .foregroundStyle(.white.opacity(0.88))
        }
    }

    private func statusColor(for percent: Int) -> Color {
        if percent >= 90 {
            return Color(red: 1.0, green: 59.0 / 255.0, blue: 48.0 / 255.0)
        }
        if percent >= 70 {
            return Color(red: 1.0, green: 149.0 / 255.0, blue: 0)
        }
        return Color(red: 52.0 / 255.0, green: 199.0 / 255.0, blue: 89.0 / 255.0)
    }
}
