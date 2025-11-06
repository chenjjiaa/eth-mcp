# Quick Start

## Prerequisites

- Rust 1.56.0+ (latest stable recommended)

## Quick Start

Make sure you're at the root of the repository, then run:

```bash
cargo build && cargo run --bin client
```

> **Note**: The code includes built-in default values for environment variables, so you can run the project without setting up a `.env` file.

## Running Other Binaries

To run other binaries, refer to the `[[bin]]` sections in `Cargo.toml` for available options. For example:

```bash
cargo run --bin server
```

Available binaries are listed in the `Cargo.toml` file under the `[[bin]]` sections.
