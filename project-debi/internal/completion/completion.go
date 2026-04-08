package completion

import (
	"devora/internal/cmdinfo"
	"io"
	"text/template"
)

// templateData holds the data passed to each shell completion template.
type templateData struct {
	BinaryName string
	Commands   []cmdinfo.Command
}

var bashTemplate = template.Must(template.New("bash").Parse(bashScript))
var zshTemplate = template.Must(template.New("zsh").Parse(zshScript))
var fishTemplate = template.Must(template.New("fish").Parse(fishScript))

// GenerateBash writes a bash completion script to w.
func GenerateBash(w io.Writer, binaryName string, commands []cmdinfo.Command) error {
	return bashTemplate.Execute(w, templateData{BinaryName: binaryName, Commands: commands})
}

// GenerateZsh writes a zsh completion script to w.
func GenerateZsh(w io.Writer, binaryName string, commands []cmdinfo.Command) error {
	return zshTemplate.Execute(w, templateData{BinaryName: binaryName, Commands: commands})
}

// GenerateFish writes a fish completion script to w.
func GenerateFish(w io.Writer, binaryName string, commands []cmdinfo.Command) error {
	return fishTemplate.Execute(w, templateData{BinaryName: binaryName, Commands: commands})
}

const bashScript = `#!/bin/bash

_{{.BinaryName}}_completions() {
    local cur prev
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"

    case "$prev" in
{{- range .Commands}}
{{- if or .Flags .ValidArgs}}
        {{.Name}}{{if .Alias}}|{{.Alias}}{{end}})
            COMPREPLY=($(compgen -W "{{range .ValidArgs}}{{.}} {{end}}{{range .Flags}}{{.Name}} {{end}}-h --help" -- "$cur"))
            return
            ;;
{{- end}}
{{- end}}
    esac

    if [[ "$COMP_CWORD" -eq 1 ]]; then
        COMPREPLY=($(compgen -W "{{range $i, $cmd := .Commands}}{{if $i}} {{end}}{{$cmd.Name}}{{if $cmd.Alias}} {{$cmd.Alias}}{{end}}{{end}}" -- "$cur"))
    fi
}

complete -F _{{.BinaryName}}_completions {{.BinaryName}}
`

const zshScript = `#compdef {{.BinaryName}}

_{{.BinaryName}}() {
    local -a commands
    commands=(
{{- range .Commands}}
        '{{.Name}}:{{.Description}}'
{{- if .Alias}}
        '{{.Alias}}:{{.Description}}'
{{- end}}
{{- end}}
    )

    local curcontext="$curcontext" state
    _arguments -C \
        '1:command:->command' \
        '*::arg:->args'

    case "$state" in
        command)
            _describe 'command' commands
            ;;
        args)
            case "${words[1]}" in
{{- range .Commands}}
{{- if or .Flags .ValidArgs}}
            {{.Name}}{{if .Alias}}|{{.Alias}}{{end}})
                local -a completions
                completions=(
{{- range .ValidArgs}}
                    '{{.}}'
{{- end}}
{{- range .Flags}}
                    '{{.Name}}:{{.Description}}'
{{- end}}
                    '-h:Show help'
                    '--help:Show help'
                )
                _describe 'completions' completions
                ;;
{{- end}}
{{- end}}
            esac
            ;;
    esac
}

_{{.BinaryName}} "$@"
`

const fishScript = `# Fish completion for {{.BinaryName}}
{{range .Commands -}}
complete -c {{$.BinaryName}} -f -n "__fish_use_subcommand" -a "{{.Name}}" -d "{{.Description}}"
{{if .Alias -}}
complete -c {{$.BinaryName}} -f -n "__fish_use_subcommand" -a "{{.Alias}}" -d "{{.Description}}"
{{end -}}
{{end -}}
{{- range .Commands -}}
{{- if or .Flags .ValidArgs -}}
{{- $cmd := . -}}
{{- range .ValidArgs}}
complete -c {{$.BinaryName}} -f -n "__fish_seen_subcommand_from {{$cmd.Name}}{{if $cmd.Alias}} {{$cmd.Alias}}{{end}}" -a "{{.}}"
{{- end}}
{{- range .Flags}}
complete -c {{$.BinaryName}} -f -n "__fish_seen_subcommand_from {{$cmd.Name}}{{if $cmd.Alias}} {{$cmd.Alias}}{{end}}" -a "{{.Name}}" -d "{{.Description}}"
{{- end}}
complete -c {{$.BinaryName}} -f -n "__fish_seen_subcommand_from {{$cmd.Name}}{{if $cmd.Alias}} {{$cmd.Alias}}{{end}}" -a "-h" -d "Show help"
complete -c {{$.BinaryName}} -f -n "__fish_seen_subcommand_from {{$cmd.Name}}{{if $cmd.Alias}} {{$cmd.Alias}}{{end}}" -a "--help" -d "Show help"
{{end -}}
{{- end}}`
