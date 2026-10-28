import MsgPack
import Testing

struct MsgPackTests {
	@Test(arguments: [
		MsgPack.nil, true, false, 0, 127, 128, 65_536, -1, -33, -129, -40_000, .int(Int64.min), .uint(UInt64.max),
		.float(1.5), "", "héllo 👋", .string(String(repeating: "x", count: 70_000)),
		.binary([1, 2, 3]), [1, "a", [nil]], .map([.init("k", [1, 2]), .init(1, "v")]), .ext(1, [0x05]),
	])
	func roundTrips(_ value: MsgPack) throws {
		var decoder = MsgPackDecoder()
		decoder.append(value.encoded())
		#expect(try decoder.next() == normalized(value))
		#expect(try decoder.next() == nil)
	}

	@Test func decodesAcrossArbitraryChunkBoundaries() throws {
		let messages: [MsgPack] = [[2, "redraw", [["grid_line", [1, 0, 0, [["a", 1, 3]], false]]]], [1, 7, nil, "ok"]]
		let bytes = messages.flatMap { $0.encoded() }
		var decoder = MsgPackDecoder()
		var decoded: [MsgPack] = []
		for byte in bytes {
			decoder.append([byte])
			while let value = try decoder.next() { decoded.append(value) }
		}
		#expect(decoded == messages.map(normalized))
	}

	/// Non-negative ints are encoded as unsigned, which is how Neovim sends them too.
	private func normalized(_ value: MsgPack) -> MsgPack {
		switch value {
		case .int(let v) where v >= 0: .uint(UInt64(v))
		case .array(let items): .array(items.map(normalized))
		case .map(let pairs): .map(pairs.map { .init(normalized($0.key), normalized($0.value)) })
		default: value
		}
	}
}
