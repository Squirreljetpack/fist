# Bugs

rg should read input from stdin with a separate flag for paths

drag into nav pane: ask copy or move.
drag file to system

render path relies on bat's exit status instead of file existance checks since it makes sense to also fail unreadable paths, however this incorrectly fails early exits from the pager.

Custom crate for proper copy/move handling: reference yazi for touchpoints

smarter determination for when to clear the dirsizecache: not too often so that entering + return doesn't need to recompute say ~, and also not too aggresive so that invalidation occurs at a sensible time

- maybe size resort makes sense to reset cursor to 0 as well

Configurable ignore which applies unless visibility.all for nav pane
