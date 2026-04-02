import CoreServices
import Foundation

public final class FileWatcherService: @unchecked Sendable {
    public enum Event: Sendable {
        case sessionFilesChanged
        case statuslineChanged
    }

    private final class CallbackBox {
        let callback: @Sendable (Event) -> Void

        init(callback: @escaping @Sendable (Event) -> Void) {
            self.callback = callback
        }
    }

    private let callbackBox: CallbackBox
    private let queue = DispatchQueue(label: "ccrw.filewatcher")
    private let paths: [String]
    private var stream: FSEventStreamRef?

    public init(paths: [URL], callback: @escaping @Sendable (Event) -> Void) {
        self.paths = paths.map(\.path)
        self.callbackBox = CallbackBox(callback: callback)
    }

    deinit {
        stop()
    }

    public func start() {
        guard stream == nil else {
            return
        }

        var context = FSEventStreamContext(
            version: 0,
            info: UnsafeMutableRawPointer(Unmanaged.passUnretained(callbackBox).toOpaque()),
            retain: nil,
            release: nil,
            copyDescription: nil
        )

        let latency = 0.25
        guard let createdStream = FSEventStreamCreate(
            nil,
            { _, info, count, rawPaths, _, _ in
                guard let info else {
                    return
                }
                let callbackBox = Unmanaged<CallbackBox>.fromOpaque(info).takeUnretainedValue()
                let paths = unsafeBitCast(rawPaths, to: NSArray.self) as? [String] ?? []
                for path in paths {
                    if path.contains("/.claude/projects") || path.hasSuffix(".jsonl") {
                        callbackBox.callback(.sessionFilesChanged)
                        return
                    }
                    if path.contains("/.claude/") || path.hasSuffix("/.claude") {
                        callbackBox.callback(.statuslineChanged)
                        return
                    }
                }
                if count > 0 {
                    callbackBox.callback(.sessionFilesChanged)
                }
            },
            &context,
            paths as CFArray,
            FSEventStreamEventId(kFSEventStreamEventIdSinceNow),
            latency,
            UInt32(kFSEventStreamCreateFlagFileEvents | kFSEventStreamCreateFlagUseCFTypes | kFSEventStreamCreateFlagNoDefer)
        ) else {
            return
        }

        stream = createdStream
        FSEventStreamSetDispatchQueue(createdStream, queue)
        FSEventStreamStart(createdStream)
    }

    public func stop() {
        guard let stream else {
            return
        }
        FSEventStreamStop(stream)
        FSEventStreamInvalidate(stream)
        FSEventStreamRelease(stream)
        self.stream = nil
    }
}
