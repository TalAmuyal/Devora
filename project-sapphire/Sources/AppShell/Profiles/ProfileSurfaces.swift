import SwiftUI

/// Ember's first-run welcome: a card with the profile form, which the Workspace Hub shows instead of its list while there are no profiles.
struct WelcomeCard: View {
	let form: ProfileFormModel

	var body: some View {
		ScrollView {
			VStack(alignment: .leading, spacing: 14) {
				HStack(spacing: 14) {
					Text("D")
						.font(.system(size: 24, weight: .bold, design: .rounded))
						.foregroundStyle(.white)
						.frame(width: 44, height: 44)
						.background(Color.accentColor.gradient, in: .rect(cornerRadius: 10))
					Text("Welcome to Devora").font(.title.weight(.semibold))
				}
				Text("Create your first profile to get started. A profile is an isolated scope with its own repos and workspaces — for example, one for work and one for personal projects.")
					.foregroundStyle(.secondary)
					.fixedSize(horizontal: false, vertical: true)
				ProfileFormView(model: form)
			}
			.padding(24)
			.frame(width: 520)
			.background(.background.secondary, in: .rect(cornerRadius: 14))
			.overlay(alignment: .top) {
				UnevenRoundedRectangle(topLeadingRadius: 14, topTrailingRadius: 14).fill(Color.accentColor).frame(height: 3)
			}
			.shadow(radius: 12)
			.padding(20)
			.frame(maxWidth: .infinity)
		}
		.frame(maxWidth: .infinity, maxHeight: .infinity)
		.accessibilityElement(children: .contain)
		.accessibilityIdentifier(Self.identifier)
	}

	static let identifier = "first-run-welcome"
}

/// The profile form in a dialog over the Workspace Hub, which its "New Profile…" opens (`Surfaces.openNewProfileDialog`).
struct NewProfileDialog: View {
	static let layerName = "new-profile-dialog"

	let form: ProfileFormModel

	var body: some View {
		ZStack {
			Color.black.opacity(0.35)
				.contentShape(.rect)
				.onTapGesture { form.cancel() }
				.accessibilityHidden(true)
			VStack(alignment: .leading, spacing: 14) {
				Text("New Profile").font(.title3.weight(.semibold))
				ProfileFormView(model: form)
			}
			.padding(20)
			.frame(width: 500)
			.background(.windowBackground, in: .rect(cornerRadius: 12))
			.shadow(radius: 24)
			.accessibilityElement(children: .contain)
			.accessibilityIdentifier(Self.layerName)
		}
	}
}
