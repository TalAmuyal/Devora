package asana

import "testing"

func TestParseTaskURL_V1WithProjectSegment(t *testing.T) {
	url := "https://app.asana.com/1/12345/project/67890/task/111222"
	got := ParseTaskURL(url)
	if got != "111222" {
		t.Fatalf("expected %q, got %q", "111222", got)
	}
}

func TestParseTaskURL_V1WithoutProjectSegment(t *testing.T) {
	url := "https://app.asana.com/1/12345/task/111222"
	got := ParseTaskURL(url)
	if got != "111222" {
		t.Fatalf("expected %q, got %q", "111222", got)
	}
}

func TestParseTaskURL_V0(t *testing.T) {
	url := "https://app.asana.com/0/12345/999888"
	got := ParseTaskURL(url)
	if got != "999888" {
		t.Fatalf("expected %q, got %q", "999888", got)
	}
}

func TestParseTaskURL_BareNumericID(t *testing.T) {
	got := ParseTaskURL("555666")
	if got != "555666" {
		t.Fatalf("expected %q, got %q", "555666", got)
	}
}

func TestParseTaskURL_Unrecognized(t *testing.T) {
	cases := []string{
		"foo",
		"",
		"https://example.com",
		"not-a-number",
	}
	for _, in := range cases {
		if got := ParseTaskURL(in); got != "" {
			t.Errorf("ParseTaskURL(%q) = %q, want empty", in, got)
		}
	}
}

func TestTaskURL_FormatsV1(t *testing.T) {
	tr := &tracker{cfg: Config{WorkspaceID: "W-ID"}}
	got := tr.TaskURL("T-ID")
	want := "https://app.asana.com/1/W-ID/task/T-ID"
	if got != want {
		t.Fatalf("expected %q, got %q", want, got)
	}
}
