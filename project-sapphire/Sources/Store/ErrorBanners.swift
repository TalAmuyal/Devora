/// An error on screen until the user dismisses it; `count` is how many times its message was shown while it was.
public struct ErrorBanner: Identifiable, Equatable, Sendable {
	public let id: UInt64
	public let message: String
	public fileprivate(set) var count: Int
}

/// The error banners on screen, oldest first: an error whose message is already on a banner counts on it (×N) rather than adding another.
public struct ErrorBanners: Equatable, Sendable {
	public private(set) var visible: [ErrorBanner] = []
	private var lastID: UInt64 = 0

	public init() {}

	public mutating func show(_ message: String) {
		if let index = visible.firstIndex(where: { $0.message == message }) {
			visible[index].count += 1
		} else {
			lastID += 1
			visible.append(ErrorBanner(id: lastID, message: message, count: 1))
		}
	}

	public mutating func dismiss(id: UInt64) {
		visible.removeAll { $0.id == id }
	}
}
