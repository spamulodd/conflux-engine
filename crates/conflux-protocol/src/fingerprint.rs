use sha2::{Digest, Sha256};

use crate::{Credentials, Protocol, Transport};

/// Derive a stable node ID from the normalized connection fingerprint.
///
/// IDs are deterministic across process restarts and include credentials so
/// distinct endpoints are not collapsed when server/port/tag overlap.
pub fn stable_node_id(
    protocol: Protocol,
    server: &str,
    port: u16,
    credentials: &Credentials,
    transport: &Transport,
) -> String {
    let mut hasher = Sha256::new();
    for part in [
        protocol.scheme(),
        server,
        &port.to_string(),
        &credentials.fingerprint(),
        &transport.fingerprint(),
    ] {
        hasher.update(part.as_bytes());
        hasher.update([0]);
    }
    let digest = hasher.finalize();
    format!("{:016x}", u64::from_be_bytes(digest[..8].try_into().expect("8 bytes")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TransportKind;
    use uuid::Uuid;

    #[test]
    fn ids_are_stable_across_calls() {
        let creds = Credentials::Uuid {
            id: Uuid::parse_str("00000000-0000-0000-0000-000000000001").expect("uuid"),
        };
        let transport = Transport::default();
        let first = stable_node_id(Protocol::Vless, "example.com", 443, &creds, &transport);
        let second = stable_node_id(Protocol::Vless, "example.com", 443, &creds, &transport);
        assert_eq!(first, second);
    }

    #[test]
    fn distinct_credentials_produce_distinct_ids() {
        let transport = Transport::default();
        let first = stable_node_id(
            Protocol::Vless,
            "cdn.example.com",
            443,
            &Credentials::Uuid {
                id: Uuid::parse_str("11111111-1111-1111-1111-111111111111").expect("uuid"),
            },
            &transport,
        );
        let second = stable_node_id(
            Protocol::Vless,
            "cdn.example.com",
            443,
            &Credentials::Uuid {
                id: Uuid::parse_str("22222222-2222-2222-2222-222222222222").expect("uuid"),
            },
            &transport,
        );
        assert_ne!(first, second);
    }

    #[test]
    fn distinct_transport_produces_distinct_ids() {
        let creds = Credentials::Uuid {
            id: Uuid::parse_str("00000000-0000-0000-0000-000000000001").expect("uuid"),
        };
        let tcp = Transport::default();
        let mut grpc = Transport::default();
        grpc.kind = TransportKind::Grpc;
        grpc.service_name = Some("grpc".to_string());

        let first = stable_node_id(Protocol::Vless, "example.com", 443, &creds, &tcp);
        let second = stable_node_id(Protocol::Vless, "example.com", 443, &creds, &grpc);
        assert_ne!(first, second);
    }
}
