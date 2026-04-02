import AppKit

enum StatusIconFactory {
    static func image(for percent: Int) -> NSImage {
        let size = NSSize(width: 18, height: 18)
        let image = NSImage(size: size)
        image.lockFocus()

        let insetRect = NSRect(origin: .zero, size: size).insetBy(dx: 2, dy: 2)
        let path = NSBezierPath(ovalIn: insetRect)
        statusColor(for: percent).setFill()
        path.fill()

        image.unlockFocus()
        image.isTemplate = false
        return image
    }

    static func statusColor(for percent: Int) -> NSColor {
        if percent >= 90 {
            return NSColor(calibratedRed: 1.0, green: 59.0 / 255.0, blue: 48.0 / 255.0, alpha: 1)
        }
        if percent >= 70 {
            return NSColor(calibratedRed: 1.0, green: 149.0 / 255.0, blue: 0, alpha: 1)
        }
        return NSColor(calibratedRed: 52.0 / 255.0, green: 199.0 / 255.0, blue: 89.0 / 255.0, alpha: 1)
    }
}
