package asana

import "regexp"

// Pre-compiled regex patterns for ParseTaskURL. Lifted verbatim from fdev's
// internal/api/asana/client.go:20-24 so both implementations recognize the
// same set of Asana URL shapes.
var (
	taskURLV1Pattern = regexp.MustCompile(`/task/(\d+)`)
	taskURLV0Pattern = regexp.MustCompile(`/0/\d+/(\d+)`)
	taskIDPattern    = regexp.MustCompile(`^\d+$`)
)

// ParseTaskURL extracts a task ID from an Asana URL or a bare numeric ID.
//
// Recognizes:
//   - V1 format:      https://app.asana.com/1/<workspace>/project/.../task/<id>
//     (and the no-project variant https://app.asana.com/1/<workspace>/task/<id>)
//   - V0 format:      https://app.asana.com/0/<workspace>/<id>
//   - Bare numeric:   "<id>"
//
// Returns "" when the input matches none of these.
//
// Logic mirrors fdev/internal/api/asana/client.go:197-214.
func ParseTaskURL(url string) string {
	if matches := taskURLV1Pattern.FindStringSubmatch(url); len(matches) > 1 {
		return matches[1]
	}
	if matches := taskURLV0Pattern.FindStringSubmatch(url); len(matches) > 1 {
		return matches[1]
	}
	if taskIDPattern.MatchString(url) {
		return url
	}
	return ""
}
