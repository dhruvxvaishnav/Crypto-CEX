//! Versioned protocol boundary between the API and matching engine.

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
