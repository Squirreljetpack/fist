#: sh,bash,zsh,dash,ksh,ash,posix
$${Z_NAME}() {
  if [ "$#" -eq 1 ] && [ -d "$1" ]; then
    case "$1" in
      "." | "./") ;;
      *)
        cd "$1"
        return
      ;;
    esac
  fi

  unset __fist_last
  __fist_last=
  for __fist_arg do
    __fist_last="$__fist_arg"
  done

  __fist_results="$(case "$__fist_last" in
    ".") $${BINARY_PATH} :: $${Z_DOT_ARGS} --cd -- "$@" ;;
    "./") $${BINARY_PATH} :: $${Z_SLASH_ARGS} --cd -- "$@" ;;
    *)
      $${BINARY_PATH} :dir $${Z_DIR_ARGS} --cd --initial-input="$FS_INITIAL_INPUT" -- "$@"
      ;;
  esac)" || return

  if [ -d "$__fist_results" ]; then
    cd "$__fist_results" || return
  else
    echo "$__fist_results" && __fist_line="$(dirname "$__fist_results")" && [ -d "$__fist_line" ] && cd "$__fist_line" || return
  fi
}

$${NAV_NAME}() {
  FS_VERBOSITY=1 $${Z_NAME} "$@" ./;
  [ $? -eq 22 ] && FS_INITIAL_INPUT="$*" $${Z_NAME}
}

$${OPEN_NAME}() {
  # rotate the last word to the front so it can be inspected
  __fist_i=0
  while [ $((__fist_i+=1)) -lt $# ]; do
    set -- "$@" "$1"
    shift
  done

  case "${1-}" in
    # a trailing `.` word switches to the dirs table
    ".")
      shift
      $${BINARY_PATH} :dir $${OPEN_ARGS} --cd --opener="$${OPEN_CMD}" -- "$@"
    ;;
    *)
      # one more step restores the original order for the query
      set -- "$@" "$1"
      shift
      $${BINARY_PATH} :file $${OPEN_ARGS} --cd --opener="$${OPEN_CMD}" --query="$*"
    ;;
  esac
}
#:

#: fish
function $${Z_NAME}
    if test (count $argv) -eq 1 -a -d "$argv[1]"
        switch "$argv[1]"
            case "." "./"
            case "*"
                cd "$argv[1]"
                return
        end
    end

    set -l __fist_last
    if test (count $argv) -gt 0
        set __fist_last "$argv[-1]"
    end

    set -l __fist_results
    switch "$__fist_last"
        case "."
            set __fist_results ($${BINARY_PATH} :: $${Z_DOT_ARGS} --cd -- $argv)
        case "./"
            set __fist_results ($${BINARY_PATH} :: $${Z_SLASH_ARGS} --cd -- $argv)
        case "*"
            set __fist_results ($${BINARY_PATH} :dir $${Z_DIR_ARGS} --cd --initial-input="$FS_INITIAL_INPUT" -- $argv)
    end
    test $status -eq 0; or return

    if test -d "$__fist_results"
        cd "$__fist_results"; or return
    else
        echo "$__fist_results"
        set -l __fist_parent (dirname -- "$__fist_results")
        test -d "$__fist_parent"; and cd "$__fist_parent"; or return
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
    set -l __fist_q (string join " " -- $argv)
    # a trailing `.` word switches to the dirs table
    switch "$__fist_q"
        case "." "* ."
            set -e argv[-1]
            $${BINARY_PATH} :dir $${OPEN_ARGS} --cd --opener="$${OPEN_CMD}" -- $argv
        case "*"
            $${BINARY_PATH} :file $${OPEN_ARGS} --cd --opener="$${OPEN_CMD}" --query="$__fist_q"
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
  local __fist_line __fist_dir

  $${BINARY_PATH} :: $${DIRW_ARGS} --cd -- . | {
    read -r __fist_line
    [ -n "$__fist_line" ] || { zle push-line && zle accept-line; return 1; }
    if [ -d "$__fist_line" ]; then
      cd "$__fist_line"
    elif [ -f "$__fist_line" ]; then
      read -r LBUFFER <<< "$LBUFFER"
      __fist_dir="$(dirname -- "$__fist_line")" && [ -d "$__fist_dir" ] && cd "$__fist_dir" &&
      LBUFFER="${LBUFFER% } '${__fist_line:t}' " ||
      { zle push-line && zle accept-line; return 1; }
    fi
    { zle push-line && zle accept-line; }
  }
}

__fist_file_widget() {
  emulate -L zsh
  setopt localoptions pipefail
  local __fist_results __fist_item

  __fist_results="$($${BINARY_PATH} --opener="$${FILEW_CMD}" '--output-sep=\0' :: $${FILEW_ARGS})" || { zle push-line && zle accept-line; return 1; }

  read -r LBUFFER <<< "$LBUFFER"
  for __fist_item in "${(@0)__fist_results}"; do
    if [ -n "$__fist_item" ]; then
      LBUFFER="${LBUFFER% } '$__fist_item' "
    fi
  done

  { zle push-line && zle accept-line; }
}

__fist_rg_widget() {
  emulate -L zsh
  setopt localoptions pipefail
  local __fist_results __fist_item

  __fist_results="$($${BINARY_PATH} --opener="$${RGW_CMD}" '--output-sep=\0' :rg $${RGW_ARGS})" || { zle push-line && zle accept-line; return 1; }

  read -r LBUFFER <<< "$LBUFFER"
  for __fist_item in "${(@0)__fist_results}"; do
    if [ -n "$__fist_item" ]; then
      LBUFFER="${LBUFFER% } '$__fist_item' "
    fi
  done

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
  local __fist_line __fist_dir
  __fist_line="$($${BINARY_PATH} :: $${DIRW_ARGS} --cd -- .)" || return 1
  [[ -n "$__fist_line" ]] || return 1

  if [[ -d "$__fist_line" ]]; then
    cd "$__fist_line" || return 1
    # bash has no widget that accepts a line (zsh uses `zle push-line &&
    # zle accept-line`), so readline would redraw the stale prompt. Aborting
    # an empty pending line makes it draw a fresh prompt for the new
    # directory; a line with text is left untouched.
    [[ -n "$READLINE_LINE" ]] || kill -INT $$
  elif [[ -f "$__fist_line" ]]; then
    __fist_dir="$(dirname -- "$__fist_line")" && [[ -d "$__fist_dir" ]] && cd "$__fist_dir" && {
      local __fist_base="$(basename -- "$__fist_line")"
      if [[ -n "$READLINE_LINE" ]]; then
        READLINE_LINE="${READLINE_LINE% } '$__fist_base' "
      else
        READLINE_LINE="'$__fist_base' "
      fi
      READLINE_POINT=${#READLINE_LINE}
    }
  fi
}

__fist_file_widget() {
  local __fist_line
  while IFS= read -r -d '' __fist_line; do
    if [[ -n "$__fist_line" ]]; then
      if [[ -n "$READLINE_LINE" ]]; then
        READLINE_LINE="${READLINE_LINE% } '$__fist_line' "
      else
        READLINE_LINE="'$__fist_line' "
      fi
    fi
  done < <($${BINARY_PATH} --opener="$${FILEW_CMD}" '--output-sep=\0' :: $${FILEW_ARGS}) || return 1
  READLINE_POINT=${#READLINE_LINE}
}

__fist_rg_widget() {
  local __fist_line
  while IFS= read -r -d '' __fist_line; do
    if [[ -n "$__fist_line" ]]; then
      if [[ -n "$READLINE_LINE" ]]; then
        READLINE_LINE="${READLINE_LINE% } '$__fist_line' "
      else
        READLINE_LINE="'$__fist_line' "
      fi
    fi
  done < <($${BINARY_PATH} --opener="$${RGW_CMD}" '--output-sep=\0' :rg $${RGW_ARGS}) || return 1
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
    set -l __fist_line ($${BINARY_PATH} :: $${DIRW_ARGS} --cd -- .)
    test $status -eq 0; or begin; commandline -f repaint; return 1; end
    test -n "$__fist_line"; or begin; commandline -f repaint; return 1; end

    if test -d "$__fist_line"
        cd "$__fist_line"
    else if test -f "$__fist_line"
        set -l __fist_dir (dirname -- "$__fist_line")
        test -d "$__fist_dir"; and cd "$__fist_dir"
        set -l __fist_base (basename -- "$__fist_line")
        commandline -i " '$__fist_base' "
    end
    commandline -f repaint
end

function __fist_file_widget
    $${BINARY_PATH} --opener="$${FILEW_CMD}" '--output-sep=\0' :: $${FILEW_ARGS} | while read -z -l __fist_line
        test -n "$__fist_line"; and commandline -i " '$__fist_line' "
    end
    test $pipestatus[1] -eq 0; or begin; commandline -f repaint; return 1; end
    commandline -f repaint
end

function __fist_rg_widget
    $${BINARY_PATH} --opener="$${RGW_CMD}" '--output-sep=\0' :rg $${RGW_ARGS} | while read -z -l __fist_line
        test -n "$__fist_line"; and commandline -i " '$__fist_line' "
    end
    test $pipestatus[1] -eq 0; or begin; commandline -f repaint; return 1; end
    commandline -f repaint
end

test -n '$${DIRW_BIND}'; and bind '$${DIRW_BIND}' __fist_dir_widget
test -n '$${FILEW_BIND}'; and bind '$${FILEW_BIND}' __fist_file_widget
test -n '$${RGW_BIND}'; and bind '$${RGW_BIND}' __fist_rg_widget
#:

#: nu,nushell
def --env $${Z_NAME} [...args: string] {
    let __fist_count = ($args | length)
    if $__fist_count == 1 and ($args.0 | path exists) and (($args.0 | path type) == "dir") {
        if $args.0 != "." and $args.0 != "./" {
            cd $args.0
            return
        }
    }

    let __fist_last = if $__fist_count > 0 { $args | last } else { "" }
    let __fist_results = if $__fist_last == "." {
        ^$${BINARY_PATH} "::" $${Z_DOT_ARGS} --cd -- ...$args
    } else if $__fist_last == "./" {
        ^$${BINARY_PATH} "::" $${Z_SLASH_ARGS} --cd -- ...$args
    } else {
        let init_input = ($env.FS_INITIAL_INPUT? | default "")
        ^$${BINARY_PATH} ":dir" $${Z_DIR_ARGS} --cd $"--initial-input=($init_input)" -- ...$args
    }

    if ($__fist_results | is-empty) {
        return
    }

    let __fist_line = ($__fist_results | str trim)
    if ($__fist_line | is-empty) {
        return
    }

    if ($__fist_line | path exists) and (($__fist_line | path type) == "dir") {
        cd $__fist_line
    } else {
        print $__fist_line
        let __fist_parent = ($__fist_line | path dirname)
        if ($__fist_parent | path exists) and (($__fist_parent | path type) == "dir") {
            cd $__fist_parent
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

def $${OPEN_NAME} [...args: string] {
    let __fist_q = ($args | str join " ")
    # a trailing `.` word switches to the dirs table
    if $__fist_q == "." or ($__fist_q ends-with " .") {
        let __fist_rest = ($args | slice 0..<-1)
        ^$${BINARY_PATH} ":dir" $${OPEN_ARGS} "--cd" $"--opener=$${OPEN_CMD}" -- ...$__fist_rest
    } else {
        ^$${BINARY_PATH} ":file" $${OPEN_ARGS} "--cd" $"--opener=$${OPEN_CMD}" $"--query=($__fist_q)"
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
            code: {|_, __fist_dir| ^$${BINARY_PATH} ":tool" bump $__fist_dir }
        })
    }
}

def --env __fist_dir_widget [] {
    let __fist_line = (^$${BINARY_PATH} "::" $${DIRW_ARGS} --cd -- . | str trim)
    if ($__fist_line | is-empty) {
        return
    }
    if ($__fist_line | path exists) and (($__fist_line | path type) == "dir") {
        cd $__fist_line
    } else if ($__fist_line | path exists) and (($__fist_line | path type) == "file") {
        let __fist_parent = ($__fist_line | path dirname)
        if ($__fist_parent | path exists) and (($__fist_parent | path type) == "dir") {
            cd $__fist_parent
        }
        let __fist_base = ($__fist_line | path basename)
        commandline edit --insert $" '($__fist_base)' "
    }
}

def --env __fist_file_widget [] {
    let __fist_results = (^$${BINARY_PATH} $"--opener=$${FILEW_CMD}" '--output-sep=\0' "::" $${FILEW_ARGS} | str trim)
    if ($__fist_results | is-empty) {
        return
    }
    for __fist_item in ($__fist_results | split row (char nul)) {
        if not ($__fist_item | is-empty) {
            commandline edit --insert $" '($__fist_item)' "
        }
    }
}

def --env __fist_rg_widget [] {
    let __fist_results = (^$${BINARY_PATH} $"--opener=$${RGW_CMD}" '--output-sep=\0' ":rg" $${RGW_ARGS} | str trim)
    if ($__fist_results | is-empty) {
        return
    }
    for __fist_item in ($__fist_results | split row (char nul)) {
        if not ($__fist_item | is-empty) {
            commandline edit --insert $" '($__fist_item)' "
        }
    }
}

$${NU_KEYBINDINGS_BLOCK}
#:
