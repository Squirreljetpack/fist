#!/bin/sh
# Sandbox-side launcher for fs. Bound into the capture namespace at
# /home/dev/tools-repo/run-fs.sh; CommandSpace starts it as the first tab.
set -eu
export HOME=/home/dev
export XDG_CONFIG_HOME="$HOME/.config"
export XDG_STATE_HOME="$HOME/.local/state"
export XDG_CACHE_HOME="$HOME/.cache"
export XDG_DATA_HOME="$HOME/.local/share"
# nano is the predictable editor used by the Find -> Advance clip.
export EDITOR=nano
export PATH="/home/dev/repo/fist:/home/dev/ob-files:/home/dev/tools:/home/dev/local-bin:/usr/local/bin:/usr/bin:/bin"
cd "${FIST_CWD:-/home/dev/work}" || exit 1
# Shell scenes start at an interactive bash with the demo rc loaded.
if [ -n "${FIST_SHELL:-}" ]; then
  exec /bin/bash -i
fi
if [ "${FIST_ARGS:-}" = ob-files ]; then
  exec /home/dev/tools-repo/ob-files.sh
fi
if [ -n "${FIST_ARGS:-}" ]; then
  # Screenshot-only helper; arguments are authored in capture.sh.
  exec sh -c "exec /home/dev/repo/fist/fs $FIST_ARGS"
fi
exec /home/dev/repo/fist/fs
