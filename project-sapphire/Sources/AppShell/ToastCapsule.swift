import SwiftUI

/// A short-lived page toast, shared by the Workspace and Health hubs.
struct ToastCapsule: View {
	let text: String
	let identifier: String

	var body: some View {
		Text(text)
			.padding(.horizontal, 14)
			.padding(.vertical, 8)
			.background(.regularMaterial, in: .capsule)
			.shadow(radius: 4)
			.padding(.bottom, 48)
			.transition(.opacity)
			.accessibilityIdentifier(identifier)
	}
}
