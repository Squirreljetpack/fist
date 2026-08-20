# Bugs

stale row metadata entry sometimes after toggling
maybe some kind of sort races but not worth it to make airtight
rg should read input from stdin with a separate flag for paths
refactor queue execute: lots of duplication, can take Either<Builtin,Lua>
stash: copy variant: store in fist/state/stashes/name ->, parent looks for last nav_cwd and jumps there instead

drag into nav pane: ask copy or move.
drag file to system

render path relies on bat's exit status instead of file existance checks since it makes sense to also fail unreadable paths, however this incorrectly fails early exits from the pager.

Custom crate for proper copy/move handling: reference yazi for touchpoints
