#!/usr/bin/env bash
# Re-capture the README screenshots.
#
#   ./.README.assets/tools/capture.sh all
#   ./.README.assets/tools/capture.sh nav info image
#
# Each scene runs fs in a throwaway CommandSpace session managed by
# commandspace-shot, captures the window, and wraps it in a transparent
# rounded card. Raw captures stay in /tmp/fist-demo/shots; the framed PNG
# replaces .README.assets/<name>.png.
set -euo pipefail

TOOLS=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$TOOLS/../.." && pwd)
DEMO=${DEMO:-/tmp/fist-demo}
DHOME=$DEMO/home
OVERLAY=$DEMO/overlay-home
SHOTS=$DEMO/shots
ASSETS=$REPO/.README.assets
CS_DIR=${COMMANDSPACE_DIR:-$REPO/../commandspace}
OB_PRESET=${OB_PRESET:-$HOME/.config/matchmaker/presets/obsidian}

[ -d "$DHOME/work" ] || "$TOOLS/setup.sh"
[ -x "$CS_DIR/target/release/commandspace" ] || { echo "CommandSpace release binary not found" >&2; exit 1; }
[ -x "$CS_DIR/target/release/commandspace-shot" ] || { echo "build commandspace-shot first (cargo build --release -p commandspace-shot)" >&2; exit 1; }
[ -x "$OB_PRESET/ob-files" ] || { echo "ob-files not found at $OB_PRESET/ob-files" >&2; exit 1; }
[ -x "$REPO/target/release/fs" ] || { echo "build fs first: cargo build --release" >&2; exit 1; }
mkdir -p "$SHOTS" "$ASSETS"

capture() {
  local raw=$1 cwd=$2 fargs=$3 out=$4
  shift 4
  echo ">> $out"

  local -a inputs=()
  local k
  for k in "$@"; do
    case "$k" in
      text:*) inputs+=(--input "text:${k#text:}") ;;
      sleep:*) inputs+=(--input "sleep:${k#sleep:}") ;;
      *) inputs+=(--input "key:$k") ;;
    esac
  done

  # A pid namespace is the only teardown that reaps the pane's whole tree:
  # --die-with-parent only covers bwrap's direct child, so a TUI that survives
  # its terminal would otherwise be orphaned to init and spin forever.
  bwrap --dev-bind / / --die-with-parent --unshare-pid \
    --bind "$OVERLAY" /home \
    --bind "$DHOME" /home/dev \
    --ro-bind "$REPO/target/release" /home/dev/repo/fist \
    --ro-bind "$REPO" /home/dev/repo/fist-src \
    --ro-bind "$CS_DIR/target/release" /home/dev/repo/commandspace \
    --ro-bind "$OB_PRESET" /home/dev/ob-files \
    --ro-bind "$HOME/.cargo/bin" /home/dev/tools \
    --ro-bind "$HOME/.local/bin" /home/dev/local-bin \
    --ro-bind "$TOOLS" /home/dev/tools-repo \
    --setenv HOME /home/dev \
    --setenv XDG_CONFIG_HOME /home/dev/.config \
    --setenv XDG_STATE_HOME /home/dev/.local/state \
    --setenv XDG_CACHE_HOME /home/dev/.cache \
    --setenv XDG_DATA_HOME /home/dev/.local/share \
    --setenv XDG_RUNTIME_DIR /home/dev/run \
    --setenv TMPDIR /home/dev/tmp \
    --setenv PATH /home/dev/tools:/home/dev/local-bin:/usr/local/bin:/usr/bin:/bin \
    --setenv FIST_ARGS "$fargs" \
    --setenv FIST_CWD "$cwd" \
    --setenv RUST_LOG "info" \
    /home/dev/repo/commandspace/commandspace-shot \
      --out "$raw" \
      --bin /home/dev/repo/commandspace/commandspace \
      --class FistShot \
      --cwd "$cwd" \
      --screen 2000x1400x24 \
      --settle 2.5 \
      --log "${raw%.png}.log" \
      "${inputs[@]}"

  python3 "$TOOLS/frame.py" "$raw" "$ASSETS/$out"
}

scene() {
  local name=$1 raw cwd="" fargs=""
  raw=$SHOTS/$name.png
  case "$name" in
    nav)
      cwd=/home/dev/work/harborlight
      capture "$raw" "$cwd" "" nav-pane.png text:s
      ;;
    find)
      cwd=/home/dev/work/harborlight
      capture "$raw" "$cwd" "" find-pane.png ctrl+f text:api Down
      ;;
    search)
      cwd=/home/dev/work
      capture "$raw" "$cwd" "" search-pane.png \
        ctrl+r text:TODO sleep:1.3 ctrl+r text:src shift+Down shift+Down
      ;;
    custom)
      cwd=/home/dev/work
      fargs=ob-files
      capture "$raw" "$cwd" "$fargs" custom-stream.png \
        text:di sleep:0.4 Down Down sleep:0.3 ctrl+u 'text:% Prog' sleep:0.6 Down question
      ;;
    history)
      cwd=/home/dev/work
      capture "$raw" "$cwd" "" history-pane.png \
        sleep:0.5 text:vault sleep:0.3 Right sleep:0.5 ctrl+g sleep:0.8 \
        ctrl+s sleep:0.6 ctrl+g
      ;;
    apps)
      cwd=/home/dev/work/media/photos
      capture "$raw" "$cwd" "" app-pane.png text:coast alt+e text:open\ w Return
      ;;
    options)
      cwd=/home/dev/work/harborlight
      capture "$raw" "$cwd" "" options-overlay.png Down Down alt+p
      ;;
    queue)
      cwd=/home/dev/work/harborlight
      capture "$raw" "$cwd" "" queue-overlay.png \
        text:README ctrl+c ctrl+u text:Cargo.toml ctrl+x ctrl+u text:Dockerfile ctrl+c alt+u
      ;;
    info)
      cwd=/home/dev/work/media/photos
      fargs='--config /home/dev/.config/fist/config-info.toml'
      capture "$raw" "$cwd" "$fargs" lessfilter-info.png Down Down Down Down Down alt+i
      ;;
    image)
      cwd=/home/dev/work/media/photos
      capture "$raw" "$cwd" "" image-preview.png text:jpg sleep:0.4 Down Down Down
      ;;
    *)
      echo "unknown scene: $name (nav find search custom history apps options queue info image)" >&2
      exit 1
      ;;
  esac
}

if [ "${1:-all}" = all ]; then
  for s in nav find search custom history apps options queue info image; do scene "$s"; done
else
  for s in "$@"; do scene "$s"; done
fi
