import Foundation

/// What the app-wide shortcuts decided for each modifier event, appended to the file `DEVORA_INPUT_TRACE` names: the UI
/// tests set it, and attach the file when a shortcut doesn't fire, so that a failure in CI shows whether the event arrived,
/// for which window, and how the Shift double tap read it. Without the variable, nothing is written.
enum InputTrace {
	private static let file: FileHandle? = {
		guard let path = ProcessInfo.processInfo.environment["DEVORA_INPUT_TRACE"] else { return nil }
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
