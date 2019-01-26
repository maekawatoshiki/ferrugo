# Ferrugo

[![CircleCI](https://circleci.com/gh/maekawatoshiki/ferrugo.svg?style=shield)](https://circleci.com/gh/maekawatoshiki/ferrugo)
[![codecov](https://codecov.io/gh/maekawatoshiki/ferrugo/branch/master/graph/badge.svg)](https://codecov.io/gh/maekawatoshiki/ferrugo)
[![](http://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)

Ferrugo is a JVM implementation written in Rust.

*This is now just a **toy** project (for me/you to learn how it works).*



# Current Status

- Able to run some classfiles. see ``./examples/(Hello|BigInt|SmallPT).class``
- Partly support for JIT compiling powered by LLVM
- Aiming readable code (this is the hardest, yes)

# Building from Source

## Building on Linux

1. Install Rust

  Run the command below and follow the onscreen instructions. 

```sh
curl https://sh.rustup.rs -sSf | sh
```

2. Use Rust Nightly

```sh
rustup override set nightly
```

3. Install dependencies
  - LLVM 6.0
  - (Other packages as necessary...)

```sh
# e.g. Ubuntu or Debian
apt-get install llvm-6.0
```

4. Test 

```sh
cargo test
```

5. Build and Run

```sh
cargo run --release examples/Hello.class
```

## Building on other platforms

I don't know. Maybe almost the same as Linux.
