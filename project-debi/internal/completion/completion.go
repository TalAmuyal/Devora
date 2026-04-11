package completion

import (
	"devora/internal/cmdinfo"
	"io"
	"text/template"
)

// templateData holds the data passed to each shell completion template.
type templateData struct {
	BinaryName     string
	Commands       []cmdinfo.Command
	HasSubCommands bool
}

// newTemplateData constructs a templateData, computing derived fields.
func newTemplateData(binaryName string, commands []cmdinfo.Command) templateData {
	hasSubCommands := false
	for _, cmd := range commands {
		if len(cmd.SubCommands) > 0 {
			hasSubCommands = true
			break
		}
	}
	return templateData{
		BinaryName:     binaryName,
		Commands:       commands,
		HasSubCommands: hasSubCommands,
	}
}

var bashTemplate = template.Must(template.New("bash").Parse(bashScript))
var zshTemplate = template.Must(template.New("zsh").Parse(zshScript))
var fishTemplate = template.Must(template.New("fish").Parse(fishScript))

// GenerateBash writes a bash completion script to w.
func GenerateBash(w io.Writer, binaryName string, commands []cmdinfo.Command) error {
	return bashTemplate.Execute(w, newTemplateData(binaryName, commands))
}

// GenerateZsh writes a zsh completion script to w.
func GenerateZsh(w io.Writer, binaryName string, commands []cmdinfo.Command) error {
	return zshTemplate.Execute(w, newTemplateData(binaryName, commands))
}

// GenerateFish writes a fish completion script to w.
func GenerateFish(w io.Writer, binaryName string, commands []cmdinfo.Command) error {
	return fishTemplate.Execute(w, newTemplateData(binaryName, commands))
}

const bashScript = `#!/bin/bash

_{{.BinaryName}}_completions() {
    local cur
    cur="${COMP_WORDS[COMP_CWORD]}"

    # Third level (sub-command arguments)
    if [[ "$COMP_CWORD" -ge 3 ]]; then
        case "${COMP_WORDS[1]}:${COMP_WORDS[2]}" in
{{- range .Commands}}
{{- $parent := .}}
{{- range .SubCommands}}
{{- if .CompletesFiles}}
        {{$parent.Name}}:{{.Name}})
            COMPREPLY=($(compgen -f -- "$cur"))
            return
            ;;
{{- else if .Flags}}
        {{$parent.Name}}:{{.Name}})
            COMPREPLY=($(compgen -W "{{range .Flags}}{{.Name}} {{end}}-h --help" -- "$cur"))
            return
            ;;
{{- end}}
{{- end}}
{{- end}}
        esac
    fi

    # Second level (sub-commands, valid args, flags)
    if [[ "$COMP_CWORD" -eq 2 ]]; then
        case "${COMP_WORDS[1]}" in
{{- range .Commands}}
{{- if .SubCommands}}
        {{.Name}}{{if .Alias}}|{{.Alias}}{{end}})
            COMPREPLY=($(compgen -W "{{range .SubCommands}}{{.Name}} {{end}}-h --help" -- "$cur"))
            return
            ;;
{{- else if or .Flags .ValidArgs}}
        {{.Name}}{{if .Alias}}|{{.Alias}}{{end}})
            COMPREPLY=($(compgen -W "{{range .ValidArgs}}{{.}} {{end}}{{range .Flags}}{{.Name}} {{end}}-h --help" -- "$cur"))
            return
            ;;
{{- end}}
{{- end}}
        esac
    fi

    # First level (command names)
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
{{- if .SubCommands}}
            {{.Name}}{{if .Alias}}|{{.Alias}}{{end}})
                case "${words[2]}" in
{{- range .SubCommands}}
                {{.Name}})
{{- if .CompletesFiles}}
                    _files
{{- else if .Flags}}
                    local -a subcmd_completions
                    subcmd_completions=(
{{- range .Flags}}
                        '{{.Name}}:{{.Description}}'
{{- end}}
                        '-h:Show help'
                        '--help:Show help'
                    )
                    _describe 'completions' subcmd_completions
{{- end}}
                    ;;
{{- end}}
                *)
                    local -a subcmds
                    subcmds=(
{{- range .SubCommands}}
                        '{{.Name}}:{{.Description}}'
{{- end}}
                        '-h:Show help'
                        '--help:Show help'
                    )
                    _describe 'sub-commands' subcmds
                    ;;
                esac
                ;;
{{- else if or .Flags .ValidArgs}}
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
{{- if .HasSubCommands}}

function __{{.BinaryName}}_needs_subcmd_of
    set -l cmd (commandline -opc)
    if test (count $cmd) -eq 2
        for parent in $argv
            if test "$cmd[2]" = "$parent"
                return 0
            end
        end
    end
    return 1
end

function __{{.BinaryName}}_seen_subcmd
    set -l cmd (commandline -opc)
    if test (count $cmd) -ge 3
        if test "$cmd[2]" = "$argv[1]" -a "$cmd[3]" = "$argv[2]"
            return 0
        end
    end
    return 1
end
{{- end}}

{{range .Commands -}}
complete -c {{$.BinaryName}} -f -n "__fish_use_subcommand" -a "{{.Name}}" -d "{{.Description}}"
{{if .Alias -}}
complete -c {{$.BinaryName}} -f -n "__fish_use_subcommand" -a "{{.Alias}}" -d "{{.Description}}"
{{end -}}
{{end -}}
{{- range .Commands -}}
{{- if .SubCommands -}}
{{- $cmd := . -}}
{{- range .SubCommands}}
complete -c {{$.BinaryName}} -f -n "__{{$.BinaryName}}_needs_subcmd_of {{$cmd.Name}}{{if $cmd.Alias}} {{$cmd.Alias}}{{end}}" -a "{{.Name}}" -d "{{.Description}}"
{{- end}}
complete -c {{$.BinaryName}} -f -n "__{{$.BinaryName}}_needs_subcmd_of {{$cmd.Name}}{{if $cmd.Alias}} {{$cmd.Alias}}{{end}}" -a "-h" -d "Show help"
complete -c {{$.BinaryName}} -f -n "__{{$.BinaryName}}_needs_subcmd_of {{$cmd.Name}}{{if $cmd.Alias}} {{$cmd.Alias}}{{end}}" -a "--help" -d "Show help"
{{- range .SubCommands}}
{{- $sub := . -}}
{{- if .CompletesFiles}}
complete -c {{$.BinaryName}} -F -n "__{{$.BinaryName}}_seen_subcmd {{$cmd.Name}} {{.Name}}"
{{- else if .Flags}}
{{- range .Flags}}
complete -c {{$.BinaryName}} -f -n "__{{$.BinaryName}}_seen_subcmd {{$cmd.Name}} {{$sub.Name}}" -a "{{.Name}}" -d "{{.Description}}"
{{- end}}
complete -c {{$.BinaryName}} -f -n "__{{$.BinaryName}}_seen_subcmd {{$cmd.Name}} {{$sub.Name}}" -a "-h" -d "Show help"
complete -c {{$.BinaryName}} -f -n "__{{$.BinaryName}}_seen_subcmd {{$cmd.Name}} {{$sub.Name}}" -a "--help" -d "Show help"
{{- end}}
{{- end}}
{{else if or .Flags .ValidArgs -}}
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
