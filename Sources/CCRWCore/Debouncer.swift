import Foundation

public final class Debouncer: @unchecked Sendable {
    private let delay: TimeInterval
    private let queue: DispatchQueue
    private let lock = NSLock()
    private var workItem: DispatchWorkItem?

    public init(delay: TimeInterval, queue: DispatchQueue = DispatchQueue(label: "ccrw.debouncer")) {
        self.delay = delay
        self.queue = queue
    }

    public func schedule(_ action: @escaping @Sendable () -> Void) {
        lock.lock()
        defer { lock.unlock() }

        workItem?.cancel()
        let nextItem = DispatchWorkItem(block: action)
        workItem = nextItem
        queue.asyncAfter(deadline: .now() + delay, execute: nextItem)
    }

    public func cancel() {
        lock.lock()
        defer { lock.unlock() }
        workItem?.cancel()
        workItem = nil
    }
}
