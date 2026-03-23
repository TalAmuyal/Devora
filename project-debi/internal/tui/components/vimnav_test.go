package components

import "testing"

func TestVimNav_JMovesDown(t *testing.T) {
	v := VimNav{}
	called := false
	v.HandleKey("j", nil, nil, nil, func() { called = true })
	if !called {
		t.Error("expected cursorDown to be called")
	}
}

func TestVimNav_KMovesUp(t *testing.T) {
	v := VimNav{}
	called := false
	v.HandleKey("k", nil, nil, func() { called = true }, nil)
	if !called {
		t.Error("expected cursorUp to be called")
	}
}

func TestVimNav_GGGoesFirst(t *testing.T) {
	v := VimNav{}
	called := false
	v.HandleKey("g", func() { called = true }, nil, nil, nil)
	if called {
		t.Error("first g should not trigger goFirst")
	}
	v.HandleKey("g", func() { called = true }, nil, nil, nil)
	if !called {
		t.Error("gg should trigger goFirst")
	}
}

func TestVimNav_ShiftGGoesLast(t *testing.T) {
	v := VimNav{}
	called := false
	v.HandleKey("G", nil, func() { called = true }, nil, nil)
	if !called {
		t.Error("G should trigger goLast")
	}
}

func TestVimNav_OtherKeyResetsG(t *testing.T) {
	v := VimNav{}
	v.HandleKey("g", func() {}, nil, nil, nil)
	consumed := v.HandleKey("x", nil, nil, nil, nil)
	if consumed {
		t.Error("non-vim key should not be consumed")
	}
	// now gg should not trigger since g was reset
	called := false
	v.HandleKey("g", func() { called = true }, nil, nil, nil)
	if called {
		t.Error("g after reset should not trigger goFirst")
	}
}

func TestVimNav_Reset(t *testing.T) {
	v := VimNav{}
	v.HandleKey("g", func() {}, nil, nil, nil)
	v.Reset()
	called := false
	v.HandleKey("g", func() { called = true }, nil, nil, nil)
	if called {
		t.Error("g after Reset should not trigger goFirst")
	}
}

func TestVimNav_ArrowKeys(t *testing.T) {
	v := VimNav{}
	downCalled := false
	v.HandleKey("down", nil, nil, nil, func() { downCalled = true })
	if !downCalled {
		t.Error("down arrow should trigger cursorDown")
	}
	upCalled := false
	v.HandleKey("up", nil, nil, func() { upCalled = true }, nil)
	if !upCalled {
		t.Error("up arrow should trigger cursorUp")
	}
}
