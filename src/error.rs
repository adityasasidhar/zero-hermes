//! Crate-wide error type aliases.
//!
//! Anywhere a fallible function returns [`Result<T>`], the error half is
//! `anyhow::Error`. This mirrors the Python reference where every helper
//! just raises and lets the outer REPL catch.

use std::result::Result as StdResult;

/// Convenience alias for `Result<T, anyhow::Error>`.
pub type Result<T> = StdResult<T, anyhow::Error>;
