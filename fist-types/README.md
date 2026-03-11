# fist-types

Shared types and constants for the [fist](https://github.com/Squirreljetpack/fist) (Interactive Search Tool) project.

This crate provides:
- `FileCategory`: An extensive classification system for files based on extensions, filenames, and MIME types.
- `When`: A tri-state enum (`Auto`, `Always`, `Never`) used for controlling features like colorization.
- `IconMappings`: PHF-based mappings for file icons.
- Shared filter and sorting types used across different search panes.

## Features

- `serde`: Enables serialization and deserialization for the types.
- `file-format`: Integrates with the `file-format` crate for category detection based on file magic numbers.
- `clap`: Provides command-line argument parsing support for the types.
