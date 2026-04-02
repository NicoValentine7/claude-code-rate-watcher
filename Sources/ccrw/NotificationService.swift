import Foundation
import UserNotifications

@MainActor
final class NotificationService {
    private let center: UNUserNotificationCenter
    private var lastWarningSent: Date?

    init(center: UNUserNotificationCenter = .current()) {
        self.center = center
    }

    func requestAuthorization() {
        center.requestAuthorization(options: [.alert, .sound]) { _, _ in }
    }

    func checkAndNotify(usagePercent: Int, now: Date = Date()) {
        guard usagePercent >= 75 else {
            return
        }
        if let lastWarningSent, now.timeIntervalSince(lastWarningSent) < 600 {
            return
        }

        let content = UNMutableNotificationContent()
        if usagePercent >= 90 {
            content.title = "Rate limit critical"
            content.body = "Usage is at \(usagePercent)%. You may hit the limit soon."
        } else {
            content.title = "Rate limit warning"
            content.body = "Usage is at \(usagePercent)% of the estimated 5h limit."
        }
        content.sound = .default

        let request = UNNotificationRequest(
            identifier: "ccrw-threshold-\(usagePercent >= 90 ? "critical" : "warning")",
            content: content,
            trigger: nil
        )
        center.add(request)
        lastWarningSent = now
    }
}
