//! Deterministic matching engine domain core — cex-core.

pub mod book;
pub mod stop;
pub mod types;
pub mod wal;

/// Canonical crate name for diagnostics and metrics labels.
pub const CRATE_NAME: &str = "cex-core";

#[cfg(test)]
mod tests {
    use super::CRATE_NAME;

    #[test]
    fn exposes_crate_name() {
        assert_eq!(CRATE_NAME, "cex-core");
    }
}
