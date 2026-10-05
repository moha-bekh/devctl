//! Typed errors the CLI tells apart from hardware failures.

/// The user asked for something that cannot be done as written: a bad color,
/// an unknown device or region. Exits with code 2.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct ParseError(pub String);
