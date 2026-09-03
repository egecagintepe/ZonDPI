//! Zero-copy protocol parsers for IPv4/IPv6, TCP, UDP, TLS ClientHello, and HTTP.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("Packet buffer too short: {0} bytes")]
    BufferTooShort(usize),
    #[error("Invalid protocol header: {0}")]
    InvalidHeader(String),
}

/// Extracts TLS Server Name Indication (SNI) from a raw TLS ClientHello payload.
pub fn parse_tls_sni(payload: &[u8]) -> Result<Option<String>, ParseError> {
    if payload.len() < 43 {
        return Ok(None);
    }
    // Handshake Type 0x01 = ClientHello
    if payload[0] != 0x16 || payload[5] != 0x01 {
        return Ok(None);
    }
    // Basic SNI parser implementation placeholder
    Ok(None)
}
