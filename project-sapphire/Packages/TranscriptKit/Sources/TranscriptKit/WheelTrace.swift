import AppKit

/// What the transcript decided for each scroll-wheel event and each change of its visible area, appended to the file
/// `DEVORA_WHEEL_TRACE` names: the UI tests set it, and attach the file when a wheel assertion fails, so that a failure
/// in CI shows whether the event reached the transcript, how it was routed, what moved the view after it, and whether
/// its window got the display frames the scroll is animated over.
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

	/// A wheel's scroll is animated over display frames, so a window that gets none doesn't scroll (macOS stops them for a
	/// window that's occluded or off screen): notes how many frames the window of `view` got in the half second after a
	/// wheel, and whether it was visible, so that a failed wheel assertion can tell a runner that drew nothing from a
	/// transcript that didn't scroll.
	@MainActor
	static func noteFramesAfterWheel(in view: NSView) {
		guard isOn, let window = view.window else { return }
		let frames = FrameCounter()
		let link = view.displayLink(target: frames, selector: #selector(FrameCounter.tick))
		link.add(to: .main, forMode: .common)
		let visibleThen = window.occlusionState.contains(.visible)
		DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
			link.invalidate()
			note(
				"frames after the wheel: \(frames.count) in 0.5 s, window visible \(visibleThen) at the wheel and "
					+ "\(window.occlusionState.contains(.visible)) after, on screen \(window.isVisible)")
		}
	}
}

private final class FrameCounter: NSObject {
	private(set) var count = 0
	@objc func tick() { count += 1 }
}
