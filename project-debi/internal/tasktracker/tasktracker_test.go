package tasktracker

import (
	"errors"
	"strings"
	"testing"
)

// fakeTracker is a minimal Tracker used in tests.
type fakeTracker struct {
	provider string
}

func (f *fakeTracker) Provider() string                               { return f.provider }
func (f *fakeTracker) WhoAmI() (string, error)                        { return "", nil }
func (f *fakeTracker) CreateTask(CreateTaskRequest) (Task, error)     { return Task{}, nil }
func (f *fakeTracker) CompleteTask(string) error                      { return nil }
func (f *fakeTracker) ParseTaskURL(string) string                     { return "" }
func (f *fakeTracker) TaskURL(string) string                          { return "" }
func (f *fakeTracker) PRBodyPrefix(string) string                     { return "" }

// withProviderStub replaces the package-level getProvider hook for the
// duration of the test and restores it on cleanup.
func withProviderStub(t *testing.T, provider string) {
	t.Helper()
	prev := getProvider
	getProvider = func() string { return provider }
	t.Cleanup(func() { getProvider = prev })
}

// withFactory registers a factory for the test and removes it on cleanup.
func withFactory(t *testing.T, name string, f Factory) {
	t.Helper()
	Register(name, f)
	t.Cleanup(func() { delete(factories, name) })
}

func TestNewForActiveProfile_NoProviderConfigured_ReturnsNilNil(t *testing.T) {
	withProviderStub(t, "")

	tracker, err := NewForActiveProfile()
	if err != nil {
		t.Fatalf("expected nil error, got %v", err)
	}
	if tracker != nil {
		t.Fatalf("expected nil tracker, got %v", tracker)
	}
}

func TestNewForActiveProfile_KnownProvider_ReturnsTracker(t *testing.T) {
	withProviderStub(t, "fakeprov")
	want := &fakeTracker{provider: "fakeprov"}
	withFactory(t, "fakeprov", func() (Tracker, error) { return want, nil })

	tracker, err := NewForActiveProfile()
	if err != nil {
		t.Fatalf("expected nil error, got %v", err)
	}
	if tracker == nil {
		t.Fatal("expected non-nil tracker")
	}
	if tracker.Provider() != "fakeprov" {
		t.Fatalf("expected provider \"fakeprov\", got %q", tracker.Provider())
	}
}

func TestNewForActiveProfile_UnknownProvider_ReturnsError(t *testing.T) {
	withProviderStub(t, "linear")

	tracker, err := NewForActiveProfile()
	if err == nil {
		t.Fatal("expected error for unknown provider")
	}
	if tracker != nil {
		t.Fatalf("expected nil tracker on error, got %v", tracker)
	}
	if !strings.Contains(err.Error(), "unknown task-tracker provider: linear") {
		t.Fatalf("expected error to mention unknown provider, got %q", err.Error())
	}
}

func TestNewForActiveProfile_FactoryError_Propagates(t *testing.T) {
	withProviderStub(t, "fakeprov")
	factoryErr := errors.New("missing workspace-id")
	withFactory(t, "fakeprov", func() (Tracker, error) { return nil, factoryErr })

	tracker, err := NewForActiveProfile()
	if err == nil {
		t.Fatal("expected error from factory")
	}
	if tracker != nil {
		t.Fatalf("expected nil tracker on error, got %v", tracker)
	}
	if !errors.Is(err, factoryErr) && err.Error() != factoryErr.Error() {
		t.Fatalf("expected factory error to propagate, got %q", err.Error())
	}
}

func TestRegister_DuplicateRegistration_Panics(t *testing.T) {
	f1 := func() (Tracker, error) { return &fakeTracker{provider: "dup"}, nil }
	f2 := func() (Tracker, error) { return &fakeTracker{provider: "dup-2"}, nil }

	withFactory(t, "dup", f1)

	defer func() {
		r := recover()
		if r == nil {
			t.Fatal("expected panic on duplicate registration")
		}
		msg, ok := r.(string)
		if !ok {
			// Some panics may use error types; accept either.
			if e, eok := r.(error); eok {
				msg = e.Error()
			} else {
				t.Fatalf("unexpected panic payload type: %T (%v)", r, r)
			}
		}
		if !strings.Contains(msg, "dup") {
			t.Fatalf("expected panic message to mention provider name, got %q", msg)
		}
	}()

	Register("dup", f2)
}
