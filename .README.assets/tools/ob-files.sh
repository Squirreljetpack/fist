#!/bin/sh
# Open the demo Obsidian vault through the user's ob-files launcher.
set -eu
O=/home/dev/work/obsidian
printf '%s\n' \
  "$O/Books" \
  "$O/KJV" \
  "$O/Chess" \
  "$O/Programming" \
  "$O/Reference" \
  "$O/Theory" |
  exec /home/dev/ob-files/ob-files
