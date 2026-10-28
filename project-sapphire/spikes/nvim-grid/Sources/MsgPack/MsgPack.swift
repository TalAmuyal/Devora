/// Just enough MessagePack for Neovim's msgpack-RPC: every type Neovim sends, a streaming decoder, and an encoder.
public enum MsgPack: Sendable, Equatable {
	case `nil`
	case bool(Bool)
	case int(Int64)
	case uint(UInt64)
	case float(Double)
	case string(String)
	case binary([UInt8])
	case array([MsgPack])
	case map([Pair])
	/// Neovim uses ext types for Buffer (0), Window (1) and Tabpage (2) handles.
	case ext(Int8, [UInt8])

	public struct Pair: Sendable, Equatable {
		public var key: MsgPack
		public var value: MsgPack

		public init(_ key: MsgPack, _ value: MsgPack) {
			self.key = key
			self.value = value
		}
	}
}

// MARK: - Accessors

extension MsgPack {
	public var int: Int? {
		switch self {
		case .int(let v): Int(v)
		case .uint(let v): Int(exactly: v)
		default: nil
		}
	}

	public var double: Double? {
		switch self {
		case .float(let v): v
		case .int(let v): Double(v)
		case .uint(let v): Double(v)
		default: nil
		}
	}

	public var string: String? {
		switch self {
		case .string(let s): s
		case .binary(let b): String(decoding: b, as: UTF8.self)
		default: nil
		}
	}

	public var bool: Bool? { if case .bool(let b) = self { b } else { nil } }
	public var array: [MsgPack]? { if case .array(let a) = self { a } else { nil } }
	public var isNil: Bool { if case .nil = self { true } else { false } }

	public var map: [Pair]? { if case .map(let m) = self { m } else { nil } }

	public subscript(key: String) -> MsgPack? {
		map?.first { $0.key.string == key }?.value
	}

	/// The integer handle inside a Buffer/Window/Tabpage ext value.
	public var extHandle: Int? {
		guard case .ext(_, let bytes) = self else { return nil }
		var decoder = MsgPackDecoder()
		decoder.append(bytes)
		return (try? decoder.next())??.int
	}
}

extension MsgPack: ExpressibleByStringLiteral, ExpressibleByIntegerLiteral, ExpressibleByArrayLiteral,
	ExpressibleByBooleanLiteral, ExpressibleByNilLiteral
{
	public init(stringLiteral value: String) { self = .string(value) }
	public init(integerLiteral value: Int64) { self = .int(value) }
	public init(arrayLiteral elements: MsgPack...) { self = .array(elements) }
	public init(booleanLiteral value: Bool) { self = .bool(value) }
	public init(nilLiteral: ()) { self = .nil }
}

// MARK: - Encoding

extension MsgPack {
	public func encoded() -> [UInt8] {
		var out: [UInt8] = []
		encode(into: &out)
		return out
	}

	public func encode(into out: inout [UInt8]) {
		switch self {
		case .nil:
			out.append(0xc0)
		case .bool(let b):
			out.append(b ? 0xc3 : 0xc2)
		case .int(let v):
			if v >= 0 {
				MsgPack.uint(UInt64(v)).encode(into: &out)
			} else if v >= -32 {
				out.append(UInt8(bitPattern: Int8(v)))
			} else if v >= Int64(Int8.min) {
				out.append(0xd0); out.append(UInt8(bitPattern: Int8(v)))
			} else if v >= Int64(Int16.min) {
				out.append(0xd1); appendBE(UInt16(bitPattern: Int16(v)), &out)
			} else if v >= Int64(Int32.min) {
				out.append(0xd2); appendBE(UInt32(bitPattern: Int32(v)), &out)
			} else {
				out.append(0xd3); appendBE(UInt64(bitPattern: v), &out)
			}
		case .uint(let v):
			if v < 0x80 {
				out.append(UInt8(v))
			} else if v <= UInt64(UInt8.max) {
				out.append(0xcc); out.append(UInt8(v))
			} else if v <= UInt64(UInt16.max) {
				out.append(0xcd); appendBE(UInt16(v), &out)
			} else if v <= UInt64(UInt32.max) {
				out.append(0xce); appendBE(UInt32(v), &out)
			} else {
				out.append(0xcf); appendBE(v, &out)
			}
		case .float(let d):
			out.append(0xcb); appendBE(d.bitPattern, &out)
		case .string(let s):
			let bytes = Array(s.utf8)
			let n = bytes.count
			if n < 32 {
				out.append(0xa0 | UInt8(n))
			} else if n <= Int(UInt8.max) {
				out.append(0xd9); out.append(UInt8(n))
			} else if n <= Int(UInt16.max) {
				out.append(0xda); appendBE(UInt16(n), &out)
			} else {
				out.append(0xdb); appendBE(UInt32(n), &out)
			}
			out.append(contentsOf: bytes)
		case .binary(let bytes):
			let n = bytes.count
			if n <= Int(UInt8.max) {
				out.append(0xc4); out.append(UInt8(n))
			} else if n <= Int(UInt16.max) {
				out.append(0xc5); appendBE(UInt16(n), &out)
			} else {
				out.append(0xc6); appendBE(UInt32(n), &out)
			}
			out.append(contentsOf: bytes)
		case .array(let items):
			let n = items.count
			if n < 16 {
				out.append(0x90 | UInt8(n))
			} else if n <= Int(UInt16.max) {
				out.append(0xdc); appendBE(UInt16(n), &out)
			} else {
				out.append(0xdd); appendBE(UInt32(n), &out)
			}
			for item in items { item.encode(into: &out) }
		case .map(let pairs):
			let n = pairs.count
			if n < 16 {
				out.append(0x80 | UInt8(n))
			} else if n <= Int(UInt16.max) {
				out.append(0xde); appendBE(UInt16(n), &out)
			} else {
				out.append(0xdf); appendBE(UInt32(n), &out)
			}
			for pair in pairs {
				pair.key.encode(into: &out)
				pair.value.encode(into: &out)
			}
		case .ext(let type, let bytes):
			switch bytes.count {
			case 1: out.append(0xd4)
			case 2: out.append(0xd5)
			case 4: out.append(0xd6)
			case 8: out.append(0xd7)
			case 16: out.append(0xd8)
			case let n where n <= Int(UInt8.max): out.append(0xc7); out.append(UInt8(n))
			case let n where n <= Int(UInt16.max): out.append(0xc8); appendBE(UInt16(n), &out)
			case let n: out.append(0xc9); appendBE(UInt32(n), &out)
			}
			out.append(UInt8(bitPattern: type))
			out.append(contentsOf: bytes)
		}
	}
}

private func appendBE<T: FixedWidthInteger>(_ value: T, _ out: inout [UInt8]) {
	withUnsafeBytes(of: value.bigEndian) { out.append(contentsOf: $0) }
}

// MARK: - Decoding

public enum MsgPackError: Error, Equatable {
	case invalidByte(UInt8)
}

/// Streaming decoder: append bytes as they arrive, then call `next()` until it returns nil (more bytes needed).
public struct MsgPackDecoder: Sendable {
	private var buffer: [UInt8] = []
	private var start = 0

	public init() {}

	public mutating func append(_ bytes: some Sequence<UInt8>) {
		if start > 0, start >= buffer.count / 2 {
			buffer.removeFirst(start)
			start = 0
		}
		buffer.append(contentsOf: bytes)
	}

	public mutating func next() throws -> MsgPack? {
		var cursor = start
		do {
			let value = try buffer.withUnsafeBufferPointer { try Self.read($0, &cursor) }
			start = cursor
			return value
		} catch Incomplete.needMore {
			return nil
		}
	}

	private enum Incomplete: Error { case needMore }

	private static func take(_ buf: UnsafeBufferPointer<UInt8>, _ i: inout Int, _ n: Int) throws -> UnsafeBufferPointer<UInt8> {
		guard i + n <= buf.count else { throw Incomplete.needMore }
		defer { i += n }
		return UnsafeBufferPointer(rebasing: buf[i..<(i + n)])
	}

	private static func be<T: FixedWidthInteger>(_ buf: UnsafeBufferPointer<UInt8>, _ i: inout Int, _: T.Type) throws -> T {
		let bytes = try take(buf, &i, MemoryLayout<T>.size)
		return bytes.reduce(T.zero) { $0 << 8 | T($1) }
	}

	private static func read(_ buf: UnsafeBufferPointer<UInt8>, _ i: inout Int) throws -> MsgPack {
		let b = try take(buf, &i, 1)[0]
		switch b {
		case 0x00...0x7f: return .uint(UInt64(b))
		case 0x80...0x8f: return try readMap(buf, &i, Int(b & 0x0f))
		case 0x90...0x9f: return try readArray(buf, &i, Int(b & 0x0f))
		case 0xa0...0xbf: return try readString(buf, &i, Int(b & 0x1f))
		case 0xc0: return .nil
		case 0xc2: return .bool(false)
		case 0xc3: return .bool(true)
		case 0xc4: return .binary(Array(try take(buf, &i, Int(try be(buf, &i, UInt8.self)))))
		case 0xc5: return .binary(Array(try take(buf, &i, Int(try be(buf, &i, UInt16.self)))))
		case 0xc6: return .binary(Array(try take(buf, &i, Int(try be(buf, &i, UInt32.self)))))
		case 0xc7: return try readExt(buf, &i, Int(try be(buf, &i, UInt8.self)))
		case 0xc8: return try readExt(buf, &i, Int(try be(buf, &i, UInt16.self)))
		case 0xc9: return try readExt(buf, &i, Int(try be(buf, &i, UInt32.self)))
		case 0xca: return .float(Double(Float(bitPattern: try be(buf, &i, UInt32.self))))
		case 0xcb: return .float(Double(bitPattern: try be(buf, &i, UInt64.self)))
		case 0xcc: return .uint(UInt64(try be(buf, &i, UInt8.self)))
		case 0xcd: return .uint(UInt64(try be(buf, &i, UInt16.self)))
		case 0xce: return .uint(UInt64(try be(buf, &i, UInt32.self)))
		case 0xcf: return .uint(try be(buf, &i, UInt64.self))
		case 0xd0: return .int(Int64(Int8(bitPattern: try be(buf, &i, UInt8.self))))
		case 0xd1: return .int(Int64(Int16(bitPattern: try be(buf, &i, UInt16.self))))
		case 0xd2: return .int(Int64(Int32(bitPattern: try be(buf, &i, UInt32.self))))
		case 0xd3: return .int(Int64(bitPattern: try be(buf, &i, UInt64.self)))
		case 0xd4: return try readExt(buf, &i, 1)
		case 0xd5: return try readExt(buf, &i, 2)
		case 0xd6: return try readExt(buf, &i, 4)
		case 0xd7: return try readExt(buf, &i, 8)
		case 0xd8: return try readExt(buf, &i, 16)
		case 0xd9: return try readString(buf, &i, Int(try be(buf, &i, UInt8.self)))
		case 0xda: return try readString(buf, &i, Int(try be(buf, &i, UInt16.self)))
		case 0xdb: return try readString(buf, &i, Int(try be(buf, &i, UInt32.self)))
		case 0xdc: return try readArray(buf, &i, Int(try be(buf, &i, UInt16.self)))
		case 0xdd: return try readArray(buf, &i, Int(try be(buf, &i, UInt32.self)))
		case 0xde: return try readMap(buf, &i, Int(try be(buf, &i, UInt16.self)))
		case 0xdf: return try readMap(buf, &i, Int(try be(buf, &i, UInt32.self)))
		case 0xe0...0xff: return .int(Int64(Int8(bitPattern: b)))
		default: throw MsgPackError.invalidByte(b)
		}
	}

	private static func readString(_ buf: UnsafeBufferPointer<UInt8>, _ i: inout Int, _ n: Int) throws -> MsgPack {
		.string(String(decoding: try take(buf, &i, n), as: UTF8.self))
	}

	private static func readExt(_ buf: UnsafeBufferPointer<UInt8>, _ i: inout Int, _ n: Int) throws -> MsgPack {
		let type = Int8(bitPattern: try take(buf, &i, 1)[0])
		return .ext(type, Array(try take(buf, &i, n)))
	}

	private static func readArray(_ buf: UnsafeBufferPointer<UInt8>, _ i: inout Int, _ n: Int) throws -> MsgPack {
		var items: [MsgPack] = []
		items.reserveCapacity(min(n, 4096))
		for _ in 0..<n { items.append(try read(buf, &i)) }
		return .array(items)
	}

	private static func readMap(_ buf: UnsafeBufferPointer<UInt8>, _ i: inout Int, _ n: Int) throws -> MsgPack {
		var pairs: [MsgPack.Pair] = []
		pairs.reserveCapacity(min(n, 4096))
		for _ in 0..<n { pairs.append(MsgPack.Pair(try read(buf, &i), try read(buf, &i))) }
		return .map(pairs)
	}
}
