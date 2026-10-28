import Foundation
import MsgPack
import Synchronization

public struct NvimRPCError: Error, CustomStringConvertible {
	public let method: String
	public let payload: MsgPack
	public var description: String { "\(method) failed: \(payload.array?.last?.string ?? "\(payload)")" }

	public init(method: String, payload: MsgPack) {
		self.method = method
		self.payload = payload
	}
}

/// One `nvim --embed` process and its msgpack-RPC channel on stdio.
///
/// Reading happens on a dedicated thread; notifications are delivered on that thread to `onNotification`, so the
/// receiver decides how and when to hop to the main actor (the grid batches until `flush`).
public final class NvimClient: Sendable {
	public typealias NotificationHandler = @Sendable (_ method: String, _ params: [MsgPack]) -> Void

	public let pid: Int32
	private let process: Process
	private let stdin: FileHandle
	private let writeLock = Mutex<Void>(())
	private let nextID = Atomic<UInt32>(1)
	private let pending = Mutex<[UInt32: (method: String, continuation: CheckedContinuation<MsgPack, Error>)]>([:])
	private let exited = Atomic<Bool>(false)
	/// Set `DEVORA_SPIKE_RPC_LOG=<file>` to append a line per RPC message (method names and timings only).
	private let log: FileHandle? = ProcessInfo.processInfo.environment["DEVORA_SPIKE_RPC_LOG"].flatMap { path in
		FileManager.default.createFile(atPath: path, contents: nil)
		return FileHandle(forWritingAtPath: path)
	}

	private func trace(_ line: @autoclosure () -> String) {
		guard let log else { return }
		log.write(Data("\(String(format: "%.3f", CFAbsoluteTimeGetCurrent().truncatingRemainder(dividingBy: 1000))) \(line())\n".utf8))
	}

	/// `environment` is merged over the current process environment.
	public init(
		program: String, arguments: [String], environment: [String: String], removing: [String] = [], cwd: URL? = nil,
		onExit: @escaping @Sendable (Int32) -> Void = { _ in },
		onNotification: @escaping NotificationHandler
	) throws {
		let process = Process()
		process.executableURL = URL(fileURLWithPath: program)
		process.arguments = arguments
		var env = ProcessInfo.processInfo.environment.merging(environment) { $1 }
		for key in removing { env.removeValue(forKey: key) }
		process.environment = env
		if let cwd { process.currentDirectoryURL = cwd }
		let input = Pipe()
		let output = Pipe()
		process.standardInput = input
		process.standardOutput = output
		process.standardError = ProcessInfo.processInfo.environment["DEVORA_SPIKE_RPC_LOG"]
			.flatMap { path -> FileHandle? in
				FileManager.default.createFile(atPath: path + ".stderr", contents: nil)
				return FileHandle(forWritingAtPath: path + ".stderr")
			} ?? FileHandle.nullDevice
		// Writing to a Neovim that has exited would otherwise kill the whole app with SIGPIPE.
		signal(SIGPIPE, SIG_IGN)
		let exitHandler = onExit
		process.terminationHandler = { [log] p in
			log?.write(Data("exit: status \(p.terminationStatus), reason \(p.terminationReason == .uncaughtSignal ? "signal" : "exit")\n".utf8))
			exitHandler(p.terminationStatus)
		}
		try process.run()

		self.process = process
		self.pid = process.processIdentifier
		self.stdin = input.fileHandleForWriting

		let reader = Thread { [weak self, fd = output.fileHandleForReading.fileDescriptor] in
			var decoder = MsgPackDecoder()
			var chunk = [UInt8](repeating: 0, count: 64 * 1024)
			while true {
				let n = chunk.withUnsafeMutableBytes { read(fd, $0.baseAddress, $0.count) }
				if n <= 0 { break }
				decoder.append(chunk[0..<n])
				while let message = try? decoder.next() {
					self?.dispatch(message, onNotification)
				}
			}
			self?.failAllPending()
		}
		reader.name = "nvim-rpc-reader"
		reader.qualityOfService = .userInteractive
		reader.start()
	}

	deinit {
		if process.isRunning { process.terminate() }
	}

	public var isRunning: Bool { process.isRunning }

	public func request(_ method: String, _ params: [MsgPack] = []) async throws -> MsgPack {
		let id = nextID.add(1, ordering: .relaxed).oldValue
		return try await withCheckedThrowingContinuation { continuation in
			pending.withLock { $0[id] = (method, continuation) }
			trace("-> request \(id) \(method)")
			send(.array([.uint(0), .uint(UInt64(id)), .string(method), .array(params)]))
		}
	}

	/// Fire-and-forget: Neovim sends no response, so ordering with later requests is still preserved.
	public func notify(_ method: String, _ params: [MsgPack] = []) {
		trace("-> notify \(method) \(params.first?.string ?? "")")
		send(.array([.uint(2), .string(method), .array(params)]))
	}

	public func kill(signal: Int32 = SIGTERM) {
		Foundation.kill(pid, signal)
	}

	public func waitUntilExit() {
		process.waitUntilExit()
	}

	/// Closes stdin (Neovim's cue to quit), then escalates to SIGKILL: Neovim can be stuck at a prompt in its exit
	/// path, where it ignores both EOF and SIGTERM.
	public func shutdown(grace: Duration = .milliseconds(500)) async {
		try? stdin.close()
		let deadline = ContinuousClock.now + grace
		while process.isRunning, ContinuousClock.now < deadline {
			try? await Task.sleep(for: .milliseconds(10))
		}
		if process.isRunning { Foundation.kill(pid, SIGKILL) }
		process.waitUntilExit()
	}

	private func send(_ message: MsgPack) {
		let bytes = message.encoded()
		writeLock.withLock { _ in
			try? stdin.write(contentsOf: bytes)
		}
	}

	private func dispatch(_ message: MsgPack, _ onNotification: NotificationHandler) {
		guard let parts = message.array, let type = parts.first?.int else { return }
		switch type {
		case 1 where parts.count == 4:
			guard let id = parts[1].int.map(UInt32.init) else { return }
			guard let entry = pending.withLock({ $0.removeValue(forKey: id) }) else { return }
			trace("<- response \(id) \(entry.method) \(parts[2].isNil ? "ok" : "error: \(parts[2].array?.last?.string ?? "")")")
			if parts[2].isNil {
				entry.continuation.resume(returning: parts[3])
			} else {
				entry.continuation.resume(throwing: NvimRPCError(method: entry.method, payload: parts[2]))
			}
		case 2 where parts.count == 3:
			trace("<- notify \(parts[1].string ?? "") \(parts[1].string == "redraw" ? (parts[2].array ?? []).compactMap { $0.array?.first?.string }.joined(separator: ",") : "")")
			onNotification(parts[1].string ?? "", parts[2].array ?? [])
		case 0 where parts.count == 4:
			trace("<- request \(parts[2].string ?? "")")
			// rpcrequest() from Neovim: the host serves none yet, so answer with an error rather than hang Neovim.
			send(.array([.uint(1), parts[1], .string("devora host serves no requests"), .nil]))
		default:
			break
		}
	}

	private func failAllPending() {
		exited.store(true, ordering: .relaxed)
		let all = pending.withLock { p in
			defer { p.removeAll() }
			return p
		}
		for (_, entry) in all {
			entry.continuation.resume(throwing: NvimRPCError(method: entry.method, payload: "nvim exited"))
		}
	}
}
