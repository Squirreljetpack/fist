#: sh,bash,zsh,dash,ksh,ash,posix
$${Z_NAME}() {
  if [ "$#" -eq 1 ] && [ -d "$1" ]; then
    case "$1" in
      "." | "./" | "..") ;;
      *)
        cd "$1"
        return
      ;;
    esac
  fi

  unset last
  if [ "$#" -gt 0 ]; then
    eval last=\${$#}
  fi

  results="$(case "$last" in
    "." | "..") $${BINARY_PATH} :: $${Z_DOT_ARGS} --cd -- "$@" ;;
    "./") $${BINARY_PATH} :: $${Z_SLASH_ARGS} --cd -- "$@" ;;
    *)
      $${BINARY_PATH} :dir $${Z_DIR_ARGS} --cd --initial-input="$FS_INITIAL_INPUT" -- "$@"
      ;;
  esac)" || return

  line="$(printf '%s\n' "$results" | head -n 1)"
  if [ -d "$line" ]; then
    cd "$line" || return
  else
    echo "$line" && line="$(dirname "$line")" && [ -d "$line" ] && cd "$line" || return
  fi
}

$${NAV_NAME}() {
  FS_VERBOSITY=1 $${Z_NAME} "$@" ./;
  [ $? -eq 22 ] && FS_INITIAL_INPUT="$*" $${Z_NAME}
}

$${OPEN_NAME}() {
  if [ "$#" -eq 0 ]; then
    $${BINARY_PATH} :t bump .
    $${OPEN_CMD} .
  elif [ -e "$1" ] && { [ "$#" -ne 1 ] || [ "$1" != "." ] && [ "$1" != "./" ]; } then
    $${BINARY_PATH} :t bump -- "$@"
    $${OPEN_CMD} "$@"
  else
    i=0 len=$#
    for last; do
      if [ $((i+=1)) = 1 ]; then set --; fi
      if [ "$i" = "$len" ]; then break; fi
      set -- "$@" "$last"
    done

    # treat arguments as keywords, browse/open best match
    case "$last" in
      ".")
         $${BINARY_PATH} --opener="$${OPEN_CMD}" :: $${Z_DOT_ARGS} "${@}" .
      ;;
      "./")
         $${BINARY_PATH} --opener="$${OPEN_CMD}" :: $${Z_SLASH_ARGS} "${@}" .
      ;;
      *)
        $${Z_NAME} "$@" "$last" && $${OPEN_CMD} .
      ;;
    esac
  fi
}
#:

#: fish
function $${Z_NAME}
    if test (count $argv) -eq 1 -a -d "$argv[1]"
        switch "$argv[1]"
            case "." "./" ".."
            case "*"
                cd "$argv[1]"
                return
        end
    end

    set -l last
    if test (count $argv) -gt 0
        set last "$argv[-1]"
    end

    set -l results
    switch "$last"
        case "." ".."
            set results ($${BINARY_PATH} :: $${Z_DOT_ARGS} --cd -- $argv)
        case "./"
            set results ($${BINARY_PATH} :: $${Z_SLASH_ARGS} --cd -- $argv)
        case "*"
            set results ($${BINARY_PATH} :dir $${Z_DIR_ARGS} --cd --initial-input="$FS_INITIAL_INPUT" -- $argv)
    end
    test $status -eq 0; or return

    set -l line "$results[1]"
    if test -d "$line"
        cd "$line"; or return
    else
        echo "$line"
        set -l parent (dirname -- "$line")
        test -d "$parent"; and cd "$parent"; or return
    end
end

function $${NAV_NAME}
    set -lx FS_VERBOSITY 1
    $${Z_NAME} $argv ./
    if test $status -eq 22
        set -lx FS_INITIAL_INPUT "$argv"
        $${Z_NAME}
    end
end

function $${OPEN_NAME}
    if test (count $argv) -eq 0
        $${BINARY_PATH} :t bump .
        $${OPEN_CMD} .
    else if test -e "$argv[1]" -a \( (count $argv) -ne 1 -o \( "$argv[1]" != "." -a "$argv[1]" != "./" \) \)
        $${BINARY_PATH} :t bump -- $argv
        $${OPEN_CMD} $argv
    else
        set -l all_args $argv
        set -l last "$all_args[-1]"
        set -l rest
        if test (count $all_args) -gt 1
            set rest $all_args[1..-2]
        end

        switch "$last"
            case "."
                $${BINARY_PATH} --opener="$${OPEN_CMD}" :: $${Z_DOT_ARGS} $rest .
            case "./"
                $${BINARY_PATH} --opener="$${OPEN_CMD}" :: $${Z_SLASH_ARGS} $rest .
            case "*"
                $${Z_NAME} $all_args; and $${OPEN_CMD} .
        end
    end
end
#:

#: zsh
__fist_jump_hook() {
  if ! (( ZSH_SUBSHELL > 0 )); then
    $${BINARY_PATH} :tool bump "$PWD"
  fi
}

if [[ ${precmd_functions[(Ie)__fist_jump_hook]:-} -eq 0 ]] && [[ ${chpwd_functions[(Ie)__fist_jump_hook]:-} -eq 0 ]]; then
    chpwd_functions+=(__fist_jump_hook)
fi

__fist_dir_widget() {
  emulate -L zsh
  local line dir

  $${BINARY_PATH} :: $${DIRW_ARGS} --cd -- .. | {
    read -r line
    [ -n "$line" ] || { zle push-line && zle accept-line; return 1; }
    if [ -d "$line" ]; then
      cd "$line"
    elif [ -f "$line" ]; then
      read -r LBUFFER <<< "$LBUFFER"
      dir="$(dirname -- "$line")" && [ -d "$dir" ] && cd "$dir" &&
      LBUFFER="${LBUFFER% } '${line:t}' " ||
      { zle push-line && zle accept-line; return 1; }
    fi
    { zle push-line && zle accept-line; }
  }
}

__fist_file_widget() {
  emulate -L zsh
  setopt localoptions pipefail
  local line results

  results="$($${BINARY_PATH} --opener="$${FILEW_CMD}" :: $${FILEW_ARGS})" || { zle push-line && zle accept-line; return 1; }

  read -r LBUFFER <<< "$LBUFFER"
  while IFS= read -r line; do
    if [ -n "$line" ]; then
      LBUFFER="${LBUFFER% } '$line' "
    fi
  done <<< "$results"

  { zle push-line && zle accept-line; }
}

__fist_rg_widget() {
  emulate -L zsh
  setopt localoptions pipefail
  local line results

  results="$($${BINARY_PATH} --opener="$${RGW_CMD}" :rg $${RGW_ARGS})" || { zle push-line && zle accept-line; return 1; }

  read -r LBUFFER <<< "$LBUFFER"
  while IFS= read -r line; do
    if [ -n "$line" ]; then
      LBUFFER="${LBUFFER% } '$line' "
    fi
  done <<< "$results"

  { zle push-line && zle accept-line; }
}

zle -N __fist_dir_widget
zle -N __fist_file_widget
zle -N __fist_rg_widget

[[ -n '$${DIRW_BIND}' ]] && bindkey -M main '$${DIRW_BIND}' __fist_dir_widget
[[ -n '$${FILEW_BIND}' ]] && bindkey -M main '$${FILEW_BIND}' __fist_file_widget
[[ -n '$${RGW_BIND}' ]] && bindkey -M main '$${RGW_BIND}' __fist_rg_widget
#:

#: bash
__fist_last_pwd="$PWD"
__fist_jump_hook() {
  if [[ "$PWD" != "$__fist_last_pwd" ]]; then
    __fist_last_pwd="$PWD"
    if [[ "${BASH_SUBSHELL:-0}" -eq 0 ]]; then
      $${BINARY_PATH} :tool bump "$PWD"
    fi
  fi
}

[[ "$PROMPT_COMMAND" != *__fist_jump_hook* ]] && PROMPT_COMMAND="${PROMPT_COMMAND:+$PROMPT_COMMAND; }__fist_jump_hook"

__fist_dir_widget() {
  local line dir
  line="$($${BINARY_PATH} :: $${DIRW_ARGS} --cd -- ..)" || return 1
  [[ -n "$line" ]] || return 1

  if [[ -d "$line" ]]; then
    cd "$line" || return 1
  elif [[ -f "$line" ]]; then
    dir="$(dirname -- "$line")" && [[ -d "$dir" ]] && cd "$dir" && {
      local base="$(basename -- "$line")"
      if [[ -n "$READLINE_LINE" ]]; then
        READLINE_LINE="${READLINE_LINE% } '$base' "
      else
        READLINE_LINE="'$base' "
      fi
      READLINE_POINT=${#READLINE_LINE}
    }
  fi
}

__fist_file_widget() {
  local results line
  results="$($${BINARY_PATH} --opener="$${FILEW_CMD}" :: $${FILEW_ARGS})" || return 1

  while IFS= read -r line; do
    if [[ -n "$line" ]]; then
      if [[ -n "$READLINE_LINE" ]]; then
        READLINE_LINE="${READLINE_LINE% } '$line' "
      else
        READLINE_LINE="'$line' "
      fi
    fi
  done <<< "$results"
  READLINE_POINT=${#READLINE_LINE}
}

__fist_rg_widget() {
  local results line
  results="$($${BINARY_PATH} --opener="$${RGW_CMD}" :rg $${RGW_ARGS})" || return 1

  while IFS= read -r line; do
    if [[ -n "$line" ]]; then
      if [[ -n "$READLINE_LINE" ]]; then
        READLINE_LINE="${READLINE_LINE% } '$line' "
      else
        READLINE_LINE="'$line' "
      fi
    fi
  done <<< "$results"
  READLINE_POINT=${#READLINE_LINE}
}

[[ -n '$${DIRW_BIND}' ]] && bind -x '"$${DIRW_BIND}": __fist_dir_widget'
[[ -n '$${FILEW_BIND}' ]] && bind -x '"$${FILEW_BIND}": __fist_file_widget'
[[ -n '$${RGW_BIND}' ]] && bind -x '"$${RGW_BIND}": __fist_rg_widget'
#:

#: fish
function __fist_jump_hook --on-variable PWD
    if test "$SHLVL" -ge 1
        $${BINARY_PATH} :tool bump "$PWD"
    end
end

function __fist_dir_widget
    set -l line ($${BINARY_PATH} :: $${DIRW_ARGS} --cd -- ..)
    test $status -eq 0; or begin; commandline -f repaint; return 1; end
    test -n "$line"; or begin; commandline -f repaint; return 1; end

    if test -d "$line"
        cd "$line"
    else if test -f "$line"
        set -l dir (dirname -- "$line")
        test -d "$dir"; and cd "$dir"
        set -l base (basename -- "$line")
        commandline -i " '$base' "
    end
    commandline -f repaint
end

function __fist_file_widget
    set -l results ($${BINARY_PATH} --opener="$${FILEW_CMD}" :: $${FILEW_ARGS})
    test $status -eq 0; or begin; commandline -f repaint; return 1; end

    for line in $results
        test -n "$line"; and commandline -i " '$line' "
    end
    commandline -f repaint
end

function __fist_rg_widget
    set -l results ($${BINARY_PATH} --opener="$${RGW_CMD}" :rg $${RGW_ARGS})
    test $status -eq 0; or begin; commandline -f repaint; return 1; end

    for line in $results
        test -n "$line"; and commandline -i " '$line' "
    end
    commandline -f repaint
end

test -n '$${DIRW_BIND}'; and bind '$${DIRW_BIND}' __fist_dir_widget
test -n '$${FILEW_BIND}'; and bind '$${FILEW_BIND}' __fist_file_widget
test -n '$${RGW_BIND}'; and bind '$${RGW_BIND}' __fist_rg_widget
#:

#: nu,nushell
def --env $${Z_NAME} [...args: string] {
    let count = ($args | length)
    if $count == 1 and ($args.0 | path exists) and (($args.0 | path type) == "dir") {
        if $args.0 != "." and $args.0 != "./" and $args.0 != ".." {
            cd $args.0
            return
        }
    }

    let last = if $count > 0 { $args | last } else { "" }
    let results = if $last == "." or $last == ".." {
        ^$${BINARY_PATH} "::" $${Z_DOT_ARGS} --cd -- ...$args
    } else if $last == "./" {
        ^$${BINARY_PATH} "::" $${Z_SLASH_ARGS} --cd -- ...$args
    } else {
        let init_input = ($env.FS_INITIAL_INPUT? | default "")
        ^$${BINARY_PATH} ":dir" $${Z_DIR_ARGS} --cd $"--initial-input=($init_input)" -- ...$args
    }

    if ($results | is-empty) {
        return
    }

    let line = ($results | lines | first | str trim)
    if ($line | is-empty) {
        return
    }

    if ($line | path exists) and (($line | path type) == "dir") {
        cd $line
    } else {
        print $line
        let parent = ($line | path dirname)
        if ($parent | path exists) and (($parent | path type) == "dir") {
            cd $parent
        }
    }
}

def --env $${NAV_NAME} [...args: string] {
    with-env { FS_VERBOSITY: "1" } {
        $${Z_NAME} ...$args ./
        if ($env.LAST_EXIT_CODE? | default 0) == 22 {
            with-env { FS_INITIAL_INPUT: ($args | str join " ") } {
                $${Z_NAME}
            }
        }
    }
}

def --env $${OPEN_NAME} [...args: string] {
    let count = ($args | length)
    if $count == 0 {
        ^$${BINARY_PATH} ":t" bump .
        $${OPEN_CMD} .
    } else if ($args.0 | path exists) and ($count != 1 or ($args.0 != "." and $args.0 != "./")) {
        ^$${BINARY_PATH} ":t" bump -- ...$args
        $${OPEN_CMD} ...$args
    } else {
        let last = ($args | last)
        let rest = ($args | drop 1)

        match $last {
            "." => {
                ^$${BINARY_PATH} $"--opener=$${OPEN_CMD}" "::" $${Z_DOT_ARGS} ...$rest .
            },
            "./" => {
                ^$${BINARY_PATH} $"--opener=$${OPEN_CMD}" "::" $${Z_SLASH_ARGS} ...$rest .
            },
            _ => {
                $${Z_NAME} ...$args
                $${OPEN_CMD} .
            }
        }
    }
}

export-env {
    $env.config = (
        $env.config?
        | default {}
        | upsert hooks { default {} }
        | upsert hooks.env_change { default {} }
        | upsert hooks.env_change.PWD { default [] }
    )
    let __fist_hooked = (
        $env.config.hooks.env_change.PWD | any { try { get __fist_hook } catch { false } }
    )
    if not $__fist_hooked {
        $env.config.hooks.env_change.PWD = ($env.config.hooks.env_change.PWD | append {
            __fist_hook: true,
            code: {|_, dir| ^$${BINARY_PATH} ":tool" bump $dir }
        })
    }
}

def --env __fist_dir_widget [] {
    let line = (^$${BINARY_PATH} "::" $${DIRW_ARGS} --cd -- .. | str trim)
    if ($line | is-empty) {
        return
    }
    if ($line | path exists) and (($line | path type) == "dir") {
        cd $line
    } else if ($line | path exists) and (($line | path type) == "file") {
        let parent = ($line | path dirname)
        if ($parent | path exists) and (($parent | path type) == "dir") {
            cd $parent
        }
        let base = ($line | path basename)
        commandline edit --insert $" '($base)' "
    }
}

def --env __fist_file_widget [] {
    let results = (^$${BINARY_PATH} $"--opener=$${FILEW_CMD}" "::" $${FILEW_ARGS} | str trim)
    if ($results | is-empty) {
        return
    }
    for line in ($results | lines) {
        let item = ($line | str trim)
        if not ($item | is-empty) {
            commandline edit --insert $" '($item)' "
        }
    }
}

def --env __fist_rg_widget [] {
    let results = (^$${BINARY_PATH} $"--opener=$${RGW_CMD}" ":rg" $${RGW_ARGS} | str trim)
    if ($results | is-empty) {
        return
    }
    for line in ($results | lines) {
        let item = ($line | str trim)
        if not ($item | is-empty) {
            commandline edit --insert $" '($item)' "
        }
    }
}

$${NU_KEYBINDINGS_BLOCK}
#:
