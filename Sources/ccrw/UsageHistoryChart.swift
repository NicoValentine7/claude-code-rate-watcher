import Charts
import SwiftUI
import CCRWCore

struct UsageHistoryChart: View {
    @EnvironmentObject private var state: AppState
    @State private var selectedRange: HistoryTimeRange = .day
    @State private var chartData: [ChartDataPoint] = []
    @State private var isLoading = false

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("Usage History")
                    .font(.system(size: 13, weight: .semibold, design: .rounded))
                    .foregroundStyle(.white.opacity(0.88))
                Spacer()
                Picker("Range", selection: $selectedRange) {
                    ForEach(HistoryTimeRange.allCases, id: \.self) { range in
                        Text(range.rawValue).tag(range)
                    }
                }
                .pickerStyle(.segmented)
                .frame(width: 160)
            }

            if isLoading {
                ProgressView()
                    .frame(height: 160)
                    .frame(maxWidth: .infinity)
            } else if chartData.isEmpty {
                emptyState
            } else {
                chart
            }

            legend
        }
        .padding(14)
        .background(
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .fill(.white.opacity(0.048))
                .overlay(
                    RoundedRectangle(cornerRadius: 10, style: .continuous)
                        .strokeBorder(.white.opacity(0.045), lineWidth: 1)
                )
        )
        .onChange(of: selectedRange) { _ in
            Task { await loadData() }
        }
        .task { await loadData() }
    }

    private var chart: some View {
        Chart {
            ForEach(Array(chartData.enumerated()), id: \.offset) { _, point in
                LineMark(
                    x: .value("Time", point.date),
                    y: .value("5H %", point.fiveHour),
                    series: .value("Window", "5H")
                )
                .foregroundStyle(Color(red: 0.47, green: 0.77, blue: 0.60))
                .lineStyle(StrokeStyle(lineWidth: 2))
                .interpolationMethod(.catmullRom)

                LineMark(
                    x: .value("Time", point.date),
                    y: .value("7D %", point.sevenDay),
                    series: .value("Window", "7D")
                )
                .foregroundStyle(Color(red: 0.74, green: 0.79, blue: 0.84))
                .lineStyle(StrokeStyle(lineWidth: 1.5, dash: [4, 3]))
                .interpolationMethod(.catmullRom)
            }

            RuleMark(y: .value("Warning", 70))
                .foregroundStyle(Color(red: 0.82, green: 0.71, blue: 0.55).opacity(0.4))
                .lineStyle(StrokeStyle(lineWidth: 1, dash: [5, 5]))

            RuleMark(y: .value("Critical", 90))
                .foregroundStyle(Color(red: 0.83, green: 0.55, blue: 0.49).opacity(0.4))
                .lineStyle(StrokeStyle(lineWidth: 1, dash: [5, 5]))
        }
        .chartYScale(domain: 0...100)
        .chartYAxis {
            AxisMarks(values: [0, 25, 50, 75, 100]) { value in
                AxisGridLine(stroke: StrokeStyle(lineWidth: 0.5))
                    .foregroundStyle(.white.opacity(0.08))
                AxisValueLabel {
                    Text("\(value.as(Int.self) ?? 0)%")
                        .font(.system(size: 9, design: .monospaced))
                        .foregroundStyle(.white.opacity(0.4))
                }
            }
        }
        .chartXAxis {
            AxisMarks { value in
                AxisGridLine(stroke: StrokeStyle(lineWidth: 0.5))
                    .foregroundStyle(.white.opacity(0.06))
                AxisValueLabel {
                    if let date = value.as(Date.self) {
                        Text(xAxisLabel(for: date))
                            .font(.system(size: 9, design: .monospaced))
                            .foregroundStyle(.white.opacity(0.4))
                    }
                }
            }
        }
        .frame(height: 160)
    }

    private var emptyState: some View {
        VStack(spacing: 8) {
            Text("No history data yet")
                .font(.system(size: 13, weight: .medium, design: .rounded))
                .foregroundStyle(.white.opacity(0.5))
            Text("Usage data will appear here after monitoring begins.")
                .font(.system(size: 11, design: .rounded))
                .foregroundStyle(.white.opacity(0.3))
        }
        .frame(height: 160)
        .frame(maxWidth: .infinity)
    }

    private var legend: some View {
        HStack(spacing: 16) {
            legendItem(color: Color(red: 0.47, green: 0.77, blue: 0.60), label: "5H Window")
            legendItem(color: Color(red: 0.74, green: 0.79, blue: 0.84), label: "7D Window")
        }
    }

    private func legendItem(color: Color, label: String) -> some View {
        HStack(spacing: 6) {
            Rectangle()
                .fill(color)
                .frame(width: 16, height: 2)
            Text(label)
                .font(.system(size: 10, weight: .medium, design: .rounded))
                .foregroundStyle(.white.opacity(0.5))
        }
    }

    private func loadData() async {
        isLoading = true
        chartData = (try? await state.historyStore.chartData(for: selectedRange)) ?? []
        isLoading = false
    }

    private func xAxisLabel(for date: Date) -> String {
        let formatter = DateFormatter()
        switch selectedRange {
        case .day:
            formatter.dateFormat = "HH:mm"
        case .week:
            formatter.dateFormat = "E"
        case .month:
            formatter.dateFormat = "M/d"
        }
        return formatter.string(from: date)
    }
}
