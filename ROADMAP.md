# Bugs

drag into nav pane: ask copy or move.
drag file to system

render path relies on bat's exit status instead of file existance checks since it makes sense to also fail unreadable paths, however this incorrectly fails early exits from the pager.

editing a completed/err task resets it to pending (is this always intuitive/desirable?)

# Cursor

Parent does not restore to correct position when sort is active
New File/Dir: Try put cursor on created
(size) resort: Track item / keep at initial

# Config

Configurable ignore which applies unless visibility.all for nav pane
Flatten fd, rg into
option to toast on command completion (false default)

# lowpri

smarter determination for when to clear the dirsizecache: not too often so that entering + return doesn't need to recompute say ~, and also not too aggresive so that invalidation occurs at a sensible time
