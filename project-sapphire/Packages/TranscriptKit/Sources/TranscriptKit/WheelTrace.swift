import Foundation

/// What the transcript decided for each scroll-wheel event and each change of its visible area, appended to the file
/// `DEVORA_WHEEL_TRACE` names: the UI tests set it, and attach the file when a wheel assertion fails, so that a failure
/// in CI shows whether the event reached the transcript, how it was routed, and what moved the view after it.
/// Without the variable, nothing is written.
enum WheelTrace {
	private static let file: FileHandle? = {
		guard let path = ProcessInfo.processInfo.environment["DEVORA_WHEEL_TRACE"] else { return nil }
		FileManager.default.createFile(atPath: path, contents: nil)
		return FileHandle(forWritingAtPath: path)
	}()

	static var isOn: Bool { file != nil }

	static func note(_ line: @autoclosure () -> String) {
		guard let file else { return }
		let stamp = String(format: "%.3f", ProcessInfo.processInfo.systemUptime)
		file.write(Data("\(stamp) \(line())\n".utf8))
	}
}
