# fist-copy

Background copy, move, and archive-extraction engine for [fist](https://github.com/Squirreljetpack/fist).

Provides a worker pool with reflink support, cancellation, progress reporting, and per-format archive backends. It powers `fist`'s transfer queue and can also be used directly.
