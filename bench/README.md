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

Last measured on Ubuntu 26.04 / Rust 1.97 with `./bench/measure.sh 20`:

| metric              | target       | actual   |
| ------------------- | ------------ | -------- |
| binary size         | < 15 MB      | 4.50 MB  |
| cold start (median) | < 50 ms      | 1 ms     |
| idle RSS            | < 10 MB      | 5.6 MB   |

Re-run the script after any dependency or release-profile change; these
numbers are the whole point of the project and regressions are easy to
miss.

## Methodology notes

- Cold start uses `--version`, not `--help`. Clap + tracing init + early exit; no provider build.
- RSS uses `--mock tools`, which loads `Config::default()`, opens `Memory::open(None)` (in-memory sqlite), and registers all six tools (five builtins plus `subagent`). That's the warmup path the gateway takes before polling Telegram.
- N=30 is enough to smooth out scheduler noise without taking too long.
