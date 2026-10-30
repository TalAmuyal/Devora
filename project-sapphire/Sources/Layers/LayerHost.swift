import SwiftUI

/// Draws a stack's layers bottom to top, so draw order is stack order: a surface can't be raised on screen without being raised in its stack.
public struct LayerHost: View {
	let stack: LayerStack

	public init(stack: LayerStack) {
		self.stack = stack
	}

	public var body: some View {
		ZStack {
			ForEach(stack.layers) { layer in
				layer.spec.content
			}
		}
	}
}
