# DeRust Advanced Usage & Development Guide

## Setup

### First-time setup

You need a specific version of nightly Rust (nightly-2023-11-24) for DeRust development.

```
# Toolchain setup
rustup install nightly-2023-11-24
rustup default nightly-2023-11-24
rustup component add rustc-dev
rustup component add miri
rustup component add clippy

# Environment variable setup, put these in your `.bashrc`
export DeRust_RUST_CHANNEL=nightly-2023-11-24
export DeRust_RUNNER_HOME="<your runner home path - use setup_DeRust_runner_home.py>"

export RUSTFLAGS="-L $HOME/.rustup/toolchains/${DeRust_RUST_CHANNEL}-x86_64-unknown-linux-gnu/lib"
export LD_LIBRARY_PATH="${LD_LIBRARY_PATH}:$HOME/.rustup/toolchains/${DeRust_RUST_CHANNEL}-x86_64-unknown-linux-gnu/lib"

# Test your installation
python test.py
```

You can add `.env` file for local customization. See "Configurations" for an example.

### How to use DeRust

```
# this executes: cargo install --path "$(dirname "$0")" --force
./install-release.sh

DeRust --crate-type lib tests/unsafe_destructor/normal1.rs  # for single file testing (you need to set library include path, or use `cargo run` instead)
cargo DeRust  # for crate compilation
```

## DeRust Configurations

### DeRust

- Use `-v` or `-vv` to make logging more verbose.
  More than two v's will be ignored, and only the last option will be considered (it does not accumulate).
- If `sccache` is found in the path, it will be used to build dependencies
- `DeRust_REPORT_PATH`
  - Report file location. If set, DeRust analysis result will be serialized and
    saved to that file. Otherwise, the result will be printed to stderr.
  - If there already exists a file at the path, the existing content will be erased.
- `DeRust_LOG_PATH`
  - Log file location. If set, log will be saved to this file as well as printed to stderr.

## Development Guide

### Code Formatting

1. Follow whatever `rustfmt` does
2. Use an empty comment line if you want to bypass rustfmt's default formatting
3. Group `use` statements in order of `std` - `rustc` internals - 3rd party - local order

### Setup rust-analyzer

Run:
```
cd ..
git clone https://github.com/rust-lang/rust.git rust-nightly-2023-11-24
cd rust-nightly-2023-11-24
# Can be found with rustc --version
git checkout 6d64f7f69
git submodule init
git submodule update
```

Then, add this to the workspace setting (`.vscode/settings.json`):
```
{
    "rust-analyzer.rustcSource": "<your path to rust-nightly-2023-11-24>/Cargo.toml"
}
```
