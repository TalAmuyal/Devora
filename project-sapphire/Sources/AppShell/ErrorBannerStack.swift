import Store
import SwiftUI

/// The store's error banners, stacked at the top of the window above everything else (the layer stacks included), as Ember's `error-banner-stack`; × dismisses one.
struct ErrorBannerStack: View {
	let store: Store

	var body: some View {
		VStack(spacing: 4) {
			ForEach(store.errorBanners.visible) { banner in
				ErrorBannerView(banner: banner) { store.dispatch(.dismissError(id: banner.id)) }
			}
		}
		.padding(8)
	}
}

private struct ErrorBannerView: View {
	let banner: ErrorBanner
	let dismiss: () -> Void

	var body: some View {
		HStack(alignment: .firstTextBaseline, spacing: 8) {
			Text(banner.message)
				.textSelection(.enabled)
				.frame(maxWidth: .infinity, alignment: .leading)
			if banner.count > 1 {
				Text("×\(banner.count)").fontWeight(.semibold)
			}
			Button(action: dismiss) {
				Image(systemName: "xmark").fontWeight(.semibold)
			}
			.buttonStyle(.plain)
			.accessibilityLabel("Dismiss error")
		}
		.foregroundStyle(.white)
		.padding(.horizontal, 12)
		.padding(.vertical, 8)
		.background(.red, in: .rect(cornerRadius: 6))
		.shadow(color: .black.opacity(0.35), radius: 8, y: 4)
		.accessibilityElement(children: .contain)
		.accessibilityIdentifier("error-banner")
	}
}
