# DeRust

## Overview

DeRust is a static analysis tool for detecting type confusion bugs in Rust programs. It operates as a Rust compiler plugin that performs dataflow analysis on MIR and HIR to identify unsafe type conversions leading to three categories of bugs:

- **Out-of-bound memory access**
- **Data corruption**
- **Dangling pointers**


## Setup

### Requirements

- Rust nightly-2023-11-24 (automatically configured via `rust-toolchain.toml`)
- Operating System: Linux, macOS

### Setup and Build

```bash
# Install the required Rust toolchain
rustup install nightly-2023-11-24
rustup component add rustc-dev rustc-src llvm-tools --toolchain nightly-2023-11-24

# Build and install DeRust
./install-release.sh
```

### Usage

```bash
# Analyze a crate
cargo derust

# With verbose logging
cargo derust -v
cargo derust -vv
```


## Publication

You can find more details in our ICSE 2026 paper: [Rusted Types: Static Detection of Rust Type Confusion Bugs](https://conf.researchr.org/details/icse-2026/icse-2026-research-track/54/Rusted-Types-Static-Detection-of-Rust-Type-Confusion-Bugs)

The formalization of DeRust is available at [formal.pdf](./formal.pdf).

```
@inproceedings{icse26:derust,
  title     = {Rusted Types: Static Detection of Rust Type Confusion Bugs},
  author    = {Zhuang, Zeyang and Meng, Wei and Lyu, Michael R.},
  booktitle = {Proceedings of the 48th International Conference on Software Engineering (ICSE)},
  year      = {2026}
}
```


## Contacts

- Zeyang Zhuang (zyzhuang22@cse.cuhk.edu.hk)

