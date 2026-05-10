//! Versioned protocol boundary between the API and matching engine.
//!
//! PRD §12 freezes v1 as length-prefixed JSON over loopback TCP. This crate owns
//! the Rust representation of that protocol so API and engine processes cannot drift.

mod framing;
mod types;

pub use framing::{read_json_frame, write_json_frame, FrameError, MAX_FRAME_BYTES};
pub use types::*;

/// Current engine wire protocol version.
pub const WIRE_PROTOCOL_VERSION: u16 = 1;

#[cfg(test)]
mod tests {
    use super::WIRE_PROTOCOL_VERSION;

    #[test]
    fn exposes_wire_protocol_version() {
        assert_eq!(WIRE_PROTOCOL_VERSION, 1);
    }
}
