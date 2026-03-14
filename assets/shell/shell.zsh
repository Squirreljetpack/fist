function $${Z_NAME}() {
  local line last results #: zsh

  if (($# == 1)) && [ -d "$1" ]; then
    case "$1" in
      "." | "./" | "..") ;;
      *)
        cd "$1"
        return
      ;;
    esac
  fi

  unset last
  if    [ $# -gt 0 ]
  then  eval last=\${$#}
  fi

  results="$(case "$last" in
    "." | "..") $${BINARY_PATH} :: $${Z_DOT_ARGS} --no-read --cd -- $@ ;;
    "./") $${BINARY_PATH} :: $${Z_SLASH_ARGS} --no-read --cd -- $@ ;;
    *)
      $${BINARY_PATH} :dir $${Z_DIR_ARGS} --cd --initial-input="$FS_INITIAL_INPUT" -- $@
      ;;
  esac)" || return

  IFS= read -r line <<< "$results"
  if [ -d "$line" ]; then
    cd "$line" || return
  else
    line="$(dirname "$line")" && [ -d "$line" ] && cd "$line" || return
  fi
}

function $${OPEN_NAME}() {
  if ! (( $# )); then
    $${BINARY_PATH} :t bump .
    $${OPEN_CMD} .
  elif [ -e "$1" ] && { [ "$#" -ne 1 ] || [ "$1" != "." ] && [ "$1" != "./" ]; } then
    $${BINARY_PATH} :t bump -- $@
    $${OPEN_CMD} $@
  else
    local i len last #: zsh

    i=0 len=$#
    for last; do
      if [ $((i+=1)) = 1 ]; then set --; fi
      if [ $i = $len ]; then break; fi
      set -- "$@" "$last"
    done

    # treat arguments as keywords, browse/open best match
    case "$last" in
      ".")
        FS_OPTS="opener=[$${OPEN_CMD}] $FS_OPTS" $${BINARY_PATH} :: $${Z_DOT_ARGS} --no-read "${@}" .
      ;;
      "./")
        FS_OPTS="opener=[$${OPEN_CMD}] $FS_OPTS" $${BINARY_PATH} :: $${Z_SLASH_ARGS} --no-read "${@}" .
      ;;
      *)
        z "$@" "$last" && $${OPEN_CMD} .
      ;;
    esac
  fi
}

#: zsh
__fist_jump_hook() {
  $${BINARY_PATH} :tool bump "$PWD"
}

#: zsh
if [[ ${precmd_functions[(Ie)__fist_jump_hook]:-} -eq 0 ]] && [[ ${chpwd_functions[(Ie)__fist_jump_hook]:-} -eq 0 ]]; then
    chpwd_functions+=(__fist_jump_hook)
fi

__fist_dir_widget() {
  emulate -L zsh
  local line dir

  $${BINARY_PATH} :: $${DIRW_ARGS} --no-read --cd -- .. | {
    read -r line
    [ -n "$line" ] || { zle push-line && zle accept-line; return 1; }
    if [ -d "$line" ]; then
      cd "$line"
    elif [ -f "$line" ]; then
      dir="$(dirname "$line")" && [ -d "$dir" ] && cd "$dir" &&
      LBUFFER="${LBUFFER%% *} '$(basename "$line")' " ||
      { zle push-line && zle accept-line; return 1; }
    fi
    { zle push-line && zle accept-line; }
  }
}
__fist_file_widget() {
  emulate -L zsh
  setopt localoptions pipefail
  local line results

  results="$(FS_OPTS="opener=[$${FILEW_CMD}] $FS_OPTS" $${BINARY_PATH} :: --no-read $${FILEW_ARGS})" || { zle push-line && zle accept-line; return 1; }

  while IFS= read -r line; do
    if [ -n "$line" ]; then
      LBUFFER="${LBUFFER%% *} '$line' "
    fi
  done <<< "$results"

  { zle push-line && zle accept-line; }
}
__fist_rg_widget() {
  emulate -L zsh
  setopt localoptions pipefail

  results="$(FS_OPTS="opener=[$${RGW_CMD}] $FS_OPTS" $${BINARY_PATH} :rg $${RGW_ARGS})" || { zle push-line && zle accept-line; return 1; }

  while IFS= read -r line; do
    if [ -n "$line" ]; then
      LBUFFER="${LBUFFER%% *} '$line' "
    fi
  done <<< "$results"

  { zle push-line && zle accept-line; }
}

zle -N __fist_dir_widget
zle -N __fist_file_widget
zle -N __fist_rg_widget

bindkey -M main '$${DIRW_BIND}' __fist_dir_widget
bindkey -M main '$${FILEW_BIND}' __fist_file_widget
bindkey -M main '$${RGW_BIND}' __fist_rg_widget

#:
