import Darwin
import Foundation
import GhosttyKit
import Observation

/// One terminal: what it runs, and what Sapphire needs to know about it for display and restore.
///
/// libghostty owns the PTY and the child process; the session only describes them and reads them back.
///
/// A session exists from the moment its tab does, and runs once it has both its launch (the core's spec, which loads asynchronously) and a view in a window: libghostty then makes the surface, which spawns the child on a PTY.
/// Input that comes before that (typed keys, `typeLine`, a paste) waits in the session and goes to the surface, in order, once it exists, as typeahead does in any terminal: the PTY holds it until the shell reads it.
@MainActor
@Observable
public final class TerminalSession: Identifiable {
	public let id = UUID()
	/// What the terminal runs; nil until the core's spec has loaded.
	public private(set) var launch: TerminalLaunch?
	/// Whether libghostty runs the terminal: its surface exists, so the child has a PTY that input goes to.
	public private(set) var isRunning = false
	/// Why the terminal couldn't start, if it couldn't.
	public private(set) var startFailure: String?
	public internal(set) var title = ""
	/// The cwd the shell reported through OSC 7, if it does.
	public internal(set) var reportedPwd: String?
	public internal(set) var exitCode: Int32?
	public internal(set) var isClosed = false
	/// When the child exited, measured from the surface's creation.
	public internal(set) var runtime: Duration?

	@ObservationIgnored weak var view: TerminalView?
	@ObservationIgnored var startedAt: ContinuousClock.Instant?
	/// A focus request made before the view was in a window, claimed once it is.
	@ObservationIgnored private(set) var wantsFocus = false
	/// Input that came before the surface existed, in order.
	@ObservationIgnored private var pendingInput: [(ghostty_surface_t) -> Void] = []

	public init() {}

	/// Gives the terminal what to run; it starts once its view is in a window, or at once if it already is.
	public func start(_ launch: TerminalLaunch) {
		guard self.launch == nil, startFailure == nil else { return }
		self.launch = launch
		view?.createSurfaceIfReady()
	}

	/// Marks the terminal as unable to start, dropping the input that waited for it.
	public func fail(_ reason: String) {
		guard !isRunning else { return }
		startFailure = reason
		pendingInput = []
	}

	func surfaceCreated(_ surface: ghostty_surface_t) {
		isRunning = true
		let pending = pendingInput
		pendingInput = []
		for input in pending { input(surface) }
	}

	/// Runs `input` on the surface now, or once it exists; input for a terminal that couldn't start goes nowhere.
	func withSurface(_ input: @escaping (ghostty_surface_t) -> Void) {
		if let surface = view?.surface {
			input(surface)
		} else if startFailure == nil {
			pendingInput.append(input)
		}
	}

	func markExited(code: Int32) {
		exitCode = code
		if let startedAt { runtime = ContinuousClock.now - startedAt }
	}

	func markClosed() {
		isClosed = true
		if exitCode == nil, let startedAt { runtime = ContinuousClock.now - startedAt }
	}

	/// The process in the foreground of the terminal (the shell, or what it is running).
	public var foregroundPID: pid_t? {
		guard let surface = view?.surface else { return nil }
		let pid = ghostty_surface_foreground_pid(surface)
		return pid == 0 ? nil : pid_t(pid)
	}

	/// The foreground process's cwd, read from the kernel: works without any shell integration.
	public var currentDirectory: String? {
		guard let pid = foregroundPID else { return nil }
		var info = proc_vnodepathinfo()
		let size = Int32(MemoryLayout<proc_vnodepathinfo>.size)
		guard proc_pidinfo(pid, PROC_PIDVNODEPATHINFO, 0, &info, size) == size else { return nil }
		return withUnsafeBytes(of: info.pvi_cdir.vip_path) { raw in
			String(cString: raw.bindMemory(to: CChar.self).baseAddress!)
		}
	}

	public var ttyName: String? {
		guard let surface = view?.surface else { return nil }
		let name = ghostty_surface_tty_name(surface)
		defer { ghostty_string_free(name) }
		return name.ptr.map { String(cString: $0) }
	}

	/// Makes the terminal its window's first responder, so typed keys go to it; before its view is in a window, it does so once it is.
	public func focus() {
		guard let view, let window = view.window else {
			wantsFocus = true
			return
		}
		wantsFocus = false
		if window.firstResponder !== view { window.makeFirstResponder(view) }
	}

	/// Whether the terminal is its window's first responder, that is, where typed keys go.
	public var hasFocus: Bool {
		guard let view else { return false }
		return view.window?.firstResponder === view
	}

	/// The scrollback plus the visible screen, as text: what restore would snapshot.
	public func allText() -> String? {
		guard let surface = view?.surface else { return nil }
		let selection = ghostty_selection_s(
			top_left: ghostty_point_s(tag: GHOSTTY_POINT_SCREEN, coord: GHOSTTY_POINT_COORD_TOP_LEFT, x: 0, y: 0),
			bottom_right: ghostty_point_s(tag: GHOSTTY_POINT_SCREEN, coord: GHOSTTY_POINT_COORD_BOTTOM_RIGHT, x: 0, y: 0),
			rectangle: false)
		var text = ghostty_text_s()
		guard ghostty_surface_read_text(surface, selection, &text) else { return nil }
		defer { ghostty_surface_free_text(surface, &text) }
		guard let ptr = text.text else { return "" }
		return String(decoding: UnsafeRawBufferPointer(start: ptr, count: Int(text.text_len)), as: UTF8.self)
	}

	/// Types text into the terminal as if pasted.
	public func paste(_ text: String) {
		withSurface { surface in
			text.withCString { ghostty_surface_paste_text(surface, $0, UInt(strlen($0))) }
		}
	}

	/// Types a line and presses Return. Text alone is not enough: a trailing "\r" in `ghostty_surface_text` is not a key press.
	public func typeLine(_ line: String) {
		withSurface { surface in
			line.withCString { ghostty_surface_text(surface, $0, UInt(strlen($0))) }
			var key = ghostty_input_key_s()
			key.keycode = 0x24  // kVK_Return
			for action in [GHOSTTY_ACTION_PRESS, GHOSTTY_ACTION_RELEASE] {
				key.action = action
				_ = ghostty_surface_key(surface, key)
			}
		}
	}
}
