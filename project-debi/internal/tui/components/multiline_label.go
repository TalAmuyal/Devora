package components

import "strings"

// MultiLineLabelModel is a simple multi-line text label.
type MultiLineLabelModel struct {
	Lines []string
}

func NewMultiLineLabelModel(lines ...string) MultiLineLabelModel {
	return MultiLineLabelModel{Lines: lines}
}

func (m MultiLineLabelModel) View() string {
	return strings.Join(m.Lines, "\n")
}
