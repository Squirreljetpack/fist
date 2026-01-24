function $${Z_NAME}() {
  if (($# == 1)) && [ "$1" != . ] && [ "$1" != / ] && [ -d "$1" ]; then
    $${BINARY_PATH} :tool bump -- "$1"
    cd "$1"
    return
  fi

  local line last
  unset last
  if    [ $# -gt 0 ]
  then  eval last=\${$#}
  fi

  case "$last" in
    ".") $${BINARY_PATH} :: $${Z_DOT_ARGS} --cd -- $@ ;;
    */) $${BINARY_PATH} :: $${Z_SLASH_ARGS} --cd -- $@ ;;
    *)  $${BINARY_PATH} :dir --sort $${Z_SORT} --cd -- $@ ;;
  esac | {
    read -r line
    [ -n "$line" ] || return
    if [ -d "$line" ]; then
      cd "$line" || return
    else
      line="$(dirname "$line")" && [ -d "$line" ] && cd "$line" || return
    fi
  }
}

function $${Z_SLASH_NAME}() {
  if (($#)) {
    for last; do :; done
    case $last in
      */) ;;
      *) last="$last/";;
    esac

    $${BINARY_PATH} :: $${Z_SLASH_ARGS} --cd -- $@;
  } else {
    $${BINARY_PATH} :: $${Z_SLASH_ARGS} --cd /
  }
}

function $${ZZ_NAME}() {
  if ! (( $# )); then
    $${BINARY_PATH} :t bump .
    $${VISUAL} .
  elif [ "${@: $#}" = "." ]; then
    if (($# == 1)); then
      FS_OPTS="opener='$${VISUAL}' $FS_OPTS" $${BINARY_PATH}
    else
      z "${@:1:-1}" &&
      FS_OPTS="opener='$${VISUAL}' $FS_OPTS" $${BINARY_PATH}
    fi
  elif [[ -e $1 ]]; then
    $${BINARY_PATH} :t bump -- $@
    $${VISUAL} $@:a
  else
    z $@ &&
    $${VISUAL} .
  fi
}

__fist_jump_hook() {
  $${BINARY_PATH} :tool bump "$PWD"
}

if [[ ${precmd_functions[(Ie)__fist_jump_hook]:-} -eq 0 ]] && [[ ${chpwd_functions[(Ie)__fist_jump_hook]:-} -eq 0 ]]; then
    chpwd_functions+=(__fist_jump_hook)
fi