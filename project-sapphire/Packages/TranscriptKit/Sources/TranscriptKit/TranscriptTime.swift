import Foundation

/// When a row's item was first stored, as the row shows it, in the user's locale: only the time for a message from
/// today, the date and time for an older one, and the full date and time as its tooltip.
struct TranscriptTime: Equatable {
	/// What the row shows.
	let shown: String
	/// The tooltip's.
	let full: String

	/// `now` and `calendar` (with its locale and time zone) decide what "today" is.
	init(_ date: Date, now: Date, calendar: Calendar) {
		let locale = calendar.locale ?? .current
		func style(_ date: Date.FormatStyle.DateStyle, _ time: Date.FormatStyle.TimeStyle) -> Date.FormatStyle {
			Date.FormatStyle(date: date, time: time, locale: locale, calendar: calendar, timeZone: calendar.timeZone)
		}
		let today = calendar.isDate(date, inSameDayAs: now)
		shown = date.formatted(style(today ? .omitted : .abbreviated, .shortened))
		full = date.formatted(style(.complete, .standard))
	}
}
