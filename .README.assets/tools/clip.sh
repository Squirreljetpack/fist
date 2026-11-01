#!/usr/bin/env bash
# Record short demo clips (MP4 + poster PNG) into %%CLIPS/.
#
#   ./.README.assets/tools/clip.sh all
#   ./.README.assets/tools/clip.sh move sort filters
#
# Each scene runs in the same bwrap/CommandSpace demo world as capture.sh.
# commandspace-shot records the window with ffmpeg/x11grab while it replays
# the keys; the final frame is also framed with frame.py.
set -euo pipefail

TOOLS=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$TOOLS/../.." && pwd)
DEMO=${DEMO:-/tmp/fist-demo}
DHOME=$DEMO/home
OVERLAY=$DEMO/overlay-home
SHOTS=$DEMO/shots
LOGS=$DEMO/clips
CLIPS=$REPO/%%CLIPS
CS_DIR=${COMMANDSPACE_DIR:-$REPO/../commandspace}
OB_PRESET=${OB_PRESET:-$HOME/.config/matchmaker/presets/obsidian}

[ -d "$DHOME/work" ] || "$TOOLS/setup.sh"
[ -L "$DHOME/work/fist" ] || ln -sfn /home/dev/repo/fist-src "$DHOME/work/fist"
[ -x "$CS_DIR/target/release/commandspace" ] || { echo "CommandSpace release binary not found" >&2; exit 1; }
[ -x "$CS_DIR/target/release/commandspace-shot" ] || { echo "build commandspace-shot first (cargo build --release -p commandspace-shot)" >&2; exit 1; }
[ -x "$OB_PRESET/ob-files" ] || { echo "ob-files not found at $OB_PRESET/ob-files" >&2; exit 1; }
[ -x "$REPO/target/release/fs" ] || { echo "build fs first: cargo build --release" >&2; exit 1; }
command -v ffmpeg >/dev/null || { echo "missing dependency: ffmpeg" >&2; exit 1; }
mkdir -p "$SHOTS" "$LOGS" "$CLIPS"

clip() {
  local name=$1 cwd=$2 fargs=$3
  shift 3
  local raw=$SHOTS/$name-clip.png
  local mp4=$DEMO/clips/$name.mp4
  local out_mp4=$CLIPS/$name.mp4
  local poster=$CLIPS/$name.png
  echo ">> $name"

  local -a inputs=()
  local k
  for k in "$@"; do
    case "$k" in
      text:*) inputs+=(--input "text:${k#text:}") ;;
      sleep:*) inputs+=(--input "sleep:${k#sleep:}") ;;
      *) inputs+=(--input "key:$k") ;;
    esac
  done

  # See capture.sh: the pid namespace is what reaps the pane's grandchildren.
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
    --setenv SHELL /bin/bash \
    --setenv PATH /home/dev/repo/fist:/home/dev/tools:/home/dev/local-bin:/usr/local/bin:/usr/bin:/bin \
    --setenv EDITOR nano \
    --setenv FS_EDITOR nano \
    --setenv FIST_ARGS "$fargs" \
    --setenv FIST_CWD "$cwd" \
    --setenv FIST_SHELL "${FIST_SHELL:-}" \
    --setenv RUST_LOG "info" \
    /home/dev/repo/commandspace/commandspace-shot \
      --out "$raw" \
      --record "$mp4" \
      --bin /home/dev/repo/commandspace/commandspace \
      --class FistShot \
      --cwd "$cwd" \
      --screen 2000x1400x24 \
      --settle 2.5 \
      --key-delay "${KEY_DELAY:-0.18}" \
      --post-delay "${POST_DELAY:-0.55}" \
      --log "$LOGS/$name.log" \
      "${inputs[@]}"

  mv "$mp4" "$out_mp4"
  # Scenes whose story does not end on their poster frame can point at a
  # timestamp instead of the final frame.
  local shot=$raw
  if [ -n "${POSTER_AT:-}" ]; then
    shot=$SHOTS/$name-poster.png
    ffmpeg -v error -y -i "$out_mp4" -ss "$POSTER_AT" -frames:v 1 "$shot"
  fi
  python3 "$TOOLS/frame.py" "$shot" "$poster"
  printf '   %s  %s\n' "$out_mp4" "$(du -h "$out_mp4" | cut -f1)"
}

scene() {
  local name=$1
  POSTER_AT=
  unset FIST_SHELL
  case "$name" in
    search)
      # End by nudging the preview down a little to show the context column.
      clip search /home/dev/work "" \
        ctrl+r text:TODO sleep:1.2 ctrl+r text:src sleep:0.8 \
        shift+Down sleep:0.15 shift+Down sleep:0.4
      ;;
    custom)
      # Broad query, open the chaplet note in the editor and leave it again,
      # then filter to the Programming category and stage a new note in the
      # menu prompt without submitting it.
      clip custom /home/dev/work ob-files \
        text:di sleep:0.6 Down Down sleep:0.4 Right sleep:1.8 ctrl+x sleep:0.6 \
        ctrl+u sleep:0.2 'text:% Prog' sleep:0.9 Down sleep:0.4 \
        alt+e sleep:0.4 text:new sleep:0.3 Return sleep:0.5 \
        'text:Busty Beaver.md' sleep:1.2
      ;;
    prompt)
      # Prompt locking: Up enters the cwd prompt; ctrl-space unlocks it, and
      # Find auto-locks once a query exists. ctrl-z returns to nav and Up
      # re-enters the cwd prompt for the poster.
      clip prompt /home/dev/work "" \
        sleep:0.5 Up sleep:1.2 ctrl+space sleep:0.8 \
        ctrl+f sleep:0.6 text:auth sleep:1.2 ctrl+space sleep:0.8 ctrl+u \
        ctrl+z sleep:0.5 ctrl+u Up sleep:1.0
      ;;
    queue-edit)
      # Queue two markdown files, then delete one row in the overlay. Run the
      # survivor from the inbox pane so its empty destination means "here",
      # close the queue, delete the copied file, reopen the queue and re-run
      # the completed row (the demo config keeps completed rows after close).
      rm -f "$DHOME"/work/inbox/api.md
      clip queue-edit /home/dev/work/harborlight/docs "" \
        sleep:0.4 text:api sleep:0.3 Tab sleep:0.3 ctrl+u sleep:0.2 \
        text:log sleep:0.3 Tab sleep:0.3 ctrl+c sleep:0.6 \
        alt+u sleep:0.8 Down sleep:0.4 Delete sleep:0.9 \
        q sleep:0.5 ctrl+u sleep:0.2 \
        Left sleep:0.3 Left sleep:0.3 text:inb sleep:0.3 Right sleep:0.6 \
        alt+u sleep:0.9 Enter sleep:1.5 \
        q sleep:0.5 text:api sleep:0.3 shift+Delete sleep:0.3 text:y sleep:0.6 \
        alt+u sleep:0.9 Enter sleep:1.5
      ;;
    long-copy)
      # Copy the bulk fixture into the inbox with no destination set, then
      # watch the queue bar fill. The demo [queue.copy] buffer is tiny so the
      # transfer lasts long enough to see.
      clip long-copy /home/dev/work/data "" \
        sleep:0.4 ctrl+c sleep:0.5 \
        Left sleep:0.4 text:inb sleep:0.3 Right sleep:0.4 \
        ctrl+v sleep:0.2 question sleep:0.3 alt+u sleep:3.5
      ;;
    shell)
      # Start in the demo bash, jump with z, browse with the dir widget,
      # search and edit with the rg widget, then use the file widget to
      # stage the edited file and a pre-edited one.
      export FIST_SHELL=1
      rm -f "$DHOME"/work/harborlight/src/api/auth.rs.save
      (cd "$DHOME"/work/harborlight && git reset -q && git checkout HEAD -- src/api/auth.rs)
      clip shell /home/dev "" \
        sleep:1.2 \
        "text:z harb" Return sleep:1.5 \
        shift+Left sleep:1.0 \
        text:src sleep:0.4 Return sleep:1.2 \
        shift+Down sleep:1.0 \
        text:rotate sleep:0.8 Right sleep:2.0 \
        End sleep:0.2 "text: // reviewed" sleep:0.2 \
        ctrl+o sleep:0.5 Return sleep:0.4 ctrl+x sleep:1.5 \
        Escape sleep:1.0 \
        "text:git add " sleep:0.4 \
        shift+Right sleep:1.0 \
        text:db sleep:0.4 Tab sleep:0.4 ctrl+u sleep:0.3 \
        text:auth sleep:0.4 Tab sleep:0.4 Return sleep:1.5 \
        Return sleep:1.5 \
        "text:git status --short" Return sleep:1.5
      ;;
    history)
      # Straight from the nav pane: Folders history, its visibility toggle,
      # then Files history.
      clip history /home/dev/work "" \
        ctrl+g sleep:1.0 ctrl+s sleep:0.7 ctrl+g sleep:1.0 ctrl+s sleep:1.0
      ;;
    sort)
      # -u exposes ignored build output; ctrl+s hides .git so the size column
      # is meaningful. The Options sort pane stays open while the list
      # reorders, then changes from size to mtime before closing.
      clip sort /home/dev/work/harborlight "-u" \
        ctrl+s sleep:0.4 alt+p sleep:0.3 Right Down Down Return sleep:1.8 \
        Up Return sleep:0.6 Escape sleep:0.5
      ;;
    filters)
      # Keep the Options modal open and walk through hidden-only, hidden,
      # Dirs, Files and all so the list visibly re-filters behind it.
      clip filters /home/dev/work/harborlight "" \
        alt+p sleep:0.3 Return sleep:0.5 Return sleep:0.5 \
        Down Down sleep:0.3 Return sleep:0.5 F sleep:0.5 \
        Down sleep:0.3 Return sleep:0.6
      ;;
    image)
      # Start on the normal image preview, move across the JPGs, then scroll
      # the preview on the portrait image.
      clip image /home/dev/work/media/photos "" \
        text:jp sleep:0.8 Down sleep:0.5 Down sleep:0.5 Down sleep:0.5 \
        shift+Down sleep:0.2 shift+Down sleep:0.2 shift+Down sleep:0.5
      ;;
    stash)
      # The pane opens in the query prompt, ctrl-space leaves it, then the whole
      # scene is cursor only: stash the first docs row, Parent up to the repo,
      # two Down, Tab twice for two more entries, then open the stash pane with
      # all three.
      rm -f "$DHOME/work/harborlight/src/api/"*.7z "$DHOME/work/harborlight/docs/"*.zip
      clip stash /home/dev/work/harborlight/docs "--lock-prompt=always" \
        sleep:0.8 ctrl+space sleep:0.8 \
        Tab sleep:0.3 alt+s sleep:0.9 \
        Left sleep:0.6 Down sleep:0.3 Down sleep:0.5 \
        Tab sleep:0.3 Tab sleep:0.4 alt+s sleep:1.0 \
        alt+s sleep:2.2
      ;;
    compress)
      # Two consecutive rows in docs/; Tab selects and advances. No typed
      # file query is needed.
      rm -f "$DHOME/work/harborlight/docs/"*.zip
      clip compress /home/dev/work/harborlight/docs "" \
        Tab Tab sleep:0.3 alt+e sleep:0.3 text:zip sleep:0.3 Return sleep:1.5
      ;;
    archive)
      # Base folder shows a zip and a tar.gz. Cut a top-level file from the
      # zip, Parent back, paste at the base, then enter the tar.gz and its data/
      # subdirectory.
      rm -f "$DHOME/work/archive/README.md"
      clip archive /home/dev/work/archive "" \
        sleep:1.0 text:rel sleep:0.3 Right sleep:0.8 text:REA sleep:0.3 ctrl+x sleep:0.4 \
        Left sleep:0.6 ctrl+v sleep:0.8 ctrl+u \
        text:backup sleep:0.3 Right sleep:1.0 Right sleep:0.7 \
        Up sleep:1.5
      ;;
    move)
      mkdir -p "$DHOME/work/inbox"
      printf '# Send to archive\n\nThis note is ready for the archive.\n' > "$DHOME/work/inbox/archive-me.md"
      rm -f "$DHOME/work/archive/archive-me.md"
      clip move /home/dev/work/inbox "" \
        text:arch ctrl+x sleep:0.4 ctrl+g sleep:0.8 text:archive Right sleep:0.6 \
        ctrl+v sleep:1.0 alt+u sleep:0.8
      ;;
    previews)
      # Directory: normal preview, then Info.  SQLite: the Preview preset.
      # Images: the first item is logo.svg, whose Extended preset is the
      # text pager; after returning, the next image shows the normal image
      # preview, then Info.  No query is needed in the image section.
      clip previews /home/dev/work "" \
        text:data sleep:0.5 alt+i sleep:0.7 \
        Right sleep:0.5 text:cach sleep:0.3 question sleep:0.7 \
        Left sleep:0.4 ctrl+u text:med sleep:0.3 Right sleep:0.4 \
        text:photos sleep:0.3 Right sleep:0.4 \
        alt+l sleep:0.8 q sleep:0.4 \
        Down sleep:0.5 alt+i sleep:0.8
      ;;
    paged)
      # PagedPreview: JSON first, then the text-bearing PDF. In the pager,
      # slash opens search; space scrolls; q returns to the list.
      clip paged /home/dev/work/data "" \
        text:js sleep:0.3 ctrl+l sleep:0.8 slash text:ad Return sleep:0.5 \
        space q sleep:0.3 ctrl+u text:pdf sleep:0.3 ctrl+l sleep:1.0 \
        slash text:rel Return sleep:0.6
      ;;
    nav)
      # Second shell reel: the nav widget opens the repo folder, ctrl-f
      # switches to Find, CycleFilter narrows it to folders, a folder is
      # advanced into, the Options sort is set to name, then the second file
      # is edited.
      export FIST_SHELL=1
      rm -f "$DHOME"/work/harborlight/src/api/*.save
      clip nav /home/dev "" \
        sleep:1.2 \
        "text:x src" Return sleep:1.6 \
        ctrl+f sleep:0.8 ctrl+d sleep:0.5 ctrl+d sleep:0.8 \
        text:api sleep:0.5 shift+Right sleep:1.2 \
        alt+p sleep:0.4 Right sleep:0.3 Up Return sleep:0.7 Escape sleep:0.5 \
        Down sleep:0.4 Right sleep:1.8 \
        ctrl+x sleep:0.8
      ;;
    undo)
      # Undo/redo walk the pane stack, cursor only. --sort=name pins the rows,
      # drill three folders deep (Programming -> AI -> Multimodal), undo back
      # out, take the sibling folder, climb to the parent with Left twice, then
      # undo twice back to the sibling.
      clip undo /home/dev/work/obsidian "--sort=name" \
        sleep:0.6 \
        Down sleep:0.3 Down sleep:0.3 Down sleep:0.5 \
        Right sleep:0.8 \
        Right sleep:0.8 \
        Right sleep:0.9 \
        ctrl+z sleep:0.7 ctrl+z sleep:0.7 ctrl+z sleep:1.0 \
        Down sleep:0.3 Right sleep:0.9 \
        Left sleep:0.6 Left sleep:0.9 \
        ctrl+z sleep:0.8 ctrl+z sleep:1.2
      ;;
    undo-jump)
      # Jump to another folder with Folders history, copy a note there, then
      # undo back to the original folder and paste it in.
      rm -f "$DHOME"/work/data/quick-capture.md
      clip undo-jump /home/dev/work/data "" \
        sleep:0.5 ctrl+g sleep:0.9 text:vaul sleep:0.3 Right sleep:0.9 \
        text:sta sleep:0.3 ctrl+c sleep:0.6 \
        ctrl+z sleep:0.8 ctrl+z sleep:1.0 ctrl+v sleep:1.2
      ;;
    action)
      # Menu tour: open the action menu, scroll through it, close it without
      # choosing anything, then advance the cursor file into the editor and
      # leave the editor again. The poster is the open menu.
      POSTER_AT=1.2
      clip action /home/dev/work/harborlight/docs "" \
        alt+e sleep:0.6 Down sleep:0.3 Down sleep:0.3 Down sleep:0.6 \
        Escape sleep:0.5 Right sleep:1.8 ctrl+x sleep:0.6
      ;;
    fileops)
      rm -f "$DHOME/work/inbox/note.md" "$DHOME/work/inbox/note2.md"
      rm -rf "$DHOME/work/inbox/folder"
      clip fileops /home/dev/work/inbox "" \
        ctrl+n sleep:0.2 text:note.md Return sleep:0.5 \
        ctrl+shift+n sleep:0.2 text:folder Return sleep:0.5 \
        text:no sleep:0.2 F2 sleep:0.2 ctrl+u text:note2.md Return sleep:0.5 \
        ctrl+u text:no2 sleep:0.2 shift+Delete sleep:0.2 text:y sleep:0.5 \
        ctrl+u sleep:0.3
      ;;
    diff)
      # Two text files through diff, then two directories through compare.
      # The poster is the diff output, not the compare that ends the clip.
      POSTER_AT=2.6
      clip diff /home/dev/work/harborlight/docs "" \
        Tab Tab sleep:0.3 alt+e sleep:0.3 text:diff sleep:0.3 Return sleep:1.1 \
        q sleep:0.3 ctrl+shift+a Left sleep:0.3 text:doc sleep:0.2 Tab \
        ctrl+u text:src sleep:0.2 Tab sleep:0.2 alt+e sleep:0.3 text:com \
        sleep:0.3 Return sleep:1.1
      ;;
    *)
      echo "unknown clip: $name" >&2
      exit 1
      ;;
  esac
}

if [ $# -eq 0 ] || [ "$1" = all ]; then
  [ $# -eq 0 ] || shift
  for name in search custom history sort filters image previews paged action fileops diff stash compress archive move queue-edit long-copy prompt shell nav undo undo-jump; do
    scene "$name"
  done
else
  for name in "$@"; do
    scene "$name"
  done
fi
