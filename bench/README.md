# Bench

`measure.sh` runs the release binary and reports three numbers:

- **binary size** — `stat` on `target/release/zero-hermes`.
- **cold start** — median wall-clock of `zero-hermes --version` over N runs (default 10). `--version` is the smallest possible startup path: clap parses, version is printed, exit.
- **idle RSS** — peak resident set when running `--mock --config <tmp> tools`, which warms the same code paths the gateway uses (config load + provider build + tool registry + memory open) and then exits.

## Reproducing

```sh
cargo build --release
./bench/measure.sh 30
```

## Why these numbers

The point of the bench is to show how small an "agent" really is when you strip it to its kernel. Targets from the v0.1 brief:

| metric              | target       | actual (RTX 3050, Rust 1.97) |
| ------------------- | ------------ | ---------------------------- |
| binary size         | < 15 MB      | see `measure.sh` output      |
| cold start          | < 50 ms      | see `measure.sh` output      |
| idle RSS            | < 10 MB      | see `measure.sh` output      |

## Methodology notes

- Cold start uses `--version`, not `--help`. Clap + tracing init + early exit; no provider build.
- RSS uses `--mock tools`, which loads `Config::default()`, opens `Memory::open(None)` (in-memory sqlite), and registers all six tools. That's the warmup path the gateway takes before polling Telegram.
- N=30 is enough to smooth out scheduler noise without taking too long.
