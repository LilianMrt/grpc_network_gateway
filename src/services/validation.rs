//! Input checks every write runs before it reaches the store.
//!
//! A request that fails here returns `INVALID_ARGUMENT` and never touches the
//! database or the routing table. The bounds are the column widths of
//! `vpn_routes`, so a value that passes cannot fail the database's own length
//! check. The same numbers live in five places, which change together: these
//! consts, `proto/gateway.proto` (the record the operator's CRD validation
//! mirrors), `migrations/01_init_routing_table.sql`,
//! `k8s/12-configmap-initdb.yaml`, and Story 2.3's CRD markers.
//!
//! Lengths count characters, as Postgres `VARCHAR(n)` does, not UTF-8 bytes.
//! Messages never echo the rejected value, so they are safe to log as-is.

use std::net::Ipv4Addr;

use tonic::Status;

/// `vpn_routes.tunnel_id VARCHAR(255)` in `migrations/01_init_routing_table.sql`
/// and `k8s/12-configmap-initdb.yaml`.
pub const MAX_TUNNEL_ID_CHARS: usize = 255;

/// `vpn_routes.remote_endpoint VARCHAR(255)` in
/// `migrations/01_init_routing_table.sql` and `k8s/12-configmap-initdb.yaml`.
pub const MAX_REMOTE_ENDPOINT_CHARS: usize = 255;

/// The longest hostname RFC 1123 allows, without a trailing dot.
pub const MAX_HOSTNAME_CHARS: usize = 253;

/// The longest single hostname label.
pub const MAX_HOSTNAME_LABEL_CHARS: usize = 63;

/// Rejects an empty `owner`. The empty string is never an owner: accepting it
/// would let every ownerless caller share one identity. Its format and length
/// are deliberately not checked here (Story 1.4).
pub fn check_owner(owner: &str) -> Result<(), Status> {
    if owner.is_empty() {
        return Err(Status::invalid_argument("owner must not be empty"));
    }
    Ok(())
}

/// `tunnel_id` is 1 to [`MAX_TUNNEL_ID_CHARS`] characters.
pub fn validate_tunnel_id(tunnel_id: &str) -> Result<(), Status> {
    let chars = tunnel_id.chars().count();
    if chars == 0 || chars > MAX_TUNNEL_ID_CHARS {
        return Err(
            Status::invalid_argument(
                format!("tunnel_id must be 1 to {MAX_TUNNEL_ID_CHARS} characters, got {chars}")
            )
        );
    }
    Ok(())
}

/// `remote_endpoint` is `host:port`, at most [`MAX_REMOTE_ENDPOINT_CHARS`]
/// characters, split on the last `:`.
///
/// - port: ASCII digits only, 1 to 65535.
/// - host: a dotted IPv4 address, or an RFC 1123 hostname (dot-separated
///   labels of 1 to 63 `[A-Za-z0-9-]`, no leading or trailing `-`, at most 253
///   characters, no trailing dot). A host of only digits and dots must be a
///   valid IPv4 address. IPv6 is not accepted.
pub fn validate_remote_endpoint(remote_endpoint: &str) -> Result<(), Status> {
    check_remote_endpoint(remote_endpoint).map_err(|reason| {
        Status::invalid_argument(format!("remote_endpoint must be host:port: {reason}"))
    })
}

fn check_remote_endpoint(remote_endpoint: &str) -> Result<(), String> {
    let chars = remote_endpoint.chars().count();
    if chars > MAX_REMOTE_ENDPOINT_CHARS {
        return Err(format!("at most {MAX_REMOTE_ENDPOINT_CHARS} characters, got {chars}"));
    }
    let (host, port) = remote_endpoint.rsplit_once(':').ok_or("missing ':port'")?;
    check_port(port)?;
    check_host(host)
}

fn check_port(port: &str) -> Result<(), &'static str> {
    // Digits first: `u16::from_str` would also accept a leading '+'.
    if port.is_empty() || !port.bytes().all(|b| b.is_ascii_digit()) {
        return Err("port must be ASCII digits");
    }
    match port.parse::<u16>() {
        Ok(n) if n >= 1 => Ok(()),
        _ => Err("port must be 1 to 65535"),
    }
}

fn check_host(host: &str) -> Result<(), String> {
    if host.is_empty() {
        return Err("host must not be empty".into());
    }
    // A numeric-looking host is an IPv4 address or nothing, so `10.0.0.999`
    // is not mistaken for a hostname.
    if host.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
        return host
            .parse::<Ipv4Addr>()
            .map(|_| ())
            .map_err(|_| "host is not a valid IPv4 address".into());
    }
    // Splitting on '.' yields an empty label for a leading, doubled or
    // trailing dot, so all three are rejected by the length check.
    for label in host.split('.') {
        if label.is_empty() || label.len() > MAX_HOSTNAME_LABEL_CHARS {
            return Err(
                format!("hostname labels must be 1 to {MAX_HOSTNAME_LABEL_CHARS} characters")
            );
        }
        if !label.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
            return Err("hostname may contain only letters, digits, '-' and '.'".into());
        }
        if label.starts_with('-') || label.ends_with('-') {
            return Err("hostname labels must not start or end with '-'".into());
        }
    }
    // After the label loop, so every character is known to be ASCII and byte
    // length is character length.
    if host.len() > MAX_HOSTNAME_CHARS {
        return Err(format!("hostname must be at most {MAX_HOSTNAME_CHARS} characters"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tonic::Code;

    fn assert_invalid(result: Result<(), Status>, input: &str) {
        let status = result.expect_err(&format!("{input:?} must be rejected"));
        assert_eq!(status.code(), Code::InvalidArgument, "{input:?}: {status:?}");
    }

    #[test]
    fn owner_must_not_be_empty() {
        assert_invalid(check_owner(""), "");
        assert!(check_owner("default/tunnel-a").is_ok());
    }

    #[test]
    fn tunnel_id_bounds() {
        assert!(validate_tunnel_id("t").is_ok());
        assert!(validate_tunnel_id(&"a".repeat(MAX_TUNNEL_ID_CHARS)).is_ok());
        assert_invalid(validate_tunnel_id(""), "");
        let long = "a".repeat(MAX_TUNNEL_ID_CHARS + 1);
        assert_invalid(validate_tunnel_id(&long), &long);
    }

    #[test]
    fn tunnel_id_counts_characters_not_bytes() {
        // 255 three-byte characters: 765 bytes, still within VARCHAR(255).
        let multibyte = "\u{20AC}".repeat(MAX_TUNNEL_ID_CHARS);
        assert_eq!(multibyte.len(), 3 * MAX_TUNNEL_ID_CHARS);
        assert!(validate_tunnel_id(&multibyte).is_ok());
        let too_many = "\u{20AC}".repeat(MAX_TUNNEL_ID_CHARS + 1);
        assert_invalid(validate_tunnel_id(&too_many), &too_many);
    }

    #[test]
    fn accepts_good_endpoints() {
        for endpoint in [
            "203.0.113.7:51820",
            "vpn-1.example.com:443",
            "gw:1",
            "gw:65535",
            "1.example.com:80",
        ] {
            assert!(
                validate_remote_endpoint(endpoint).is_ok(),
                "{endpoint:?}: {:?}",
                validate_remote_endpoint(endpoint)
            );
        }
    }

    #[test]
    fn rejects_bad_endpoints() {
        for endpoint in [
            "",
            "no-port",
            "h:",
            "h:0",
            "h:70000",
            "h:+80",
            "h:8a",
            ":51820",
            "a$b:80",
            "-a.com:80",
            "a-.com:80",
            "a..com:80",
            ".a.com:80",
            "10.0.0.999:80",
            "010.0.0.1:80",
            "1.2.3:80",
            "[::1]:80",
            "::1",
            "a.com.:80",
            "h\u{e9}te.com:80",
            "a.com:80\r\n",
        ] {
            assert_invalid(validate_remote_endpoint(endpoint), endpoint);
        }
    }

    #[test]
    fn endpoint_length_bounds() {
        // 63-char labels joined by dots: 4 labels make 255 chars, too long for
        // a hostname (253), so build the bounds from shorter labels.
        let label = "a".repeat(MAX_HOSTNAME_LABEL_CHARS);
        assert!(validate_remote_endpoint(&format!("{label}:80")).is_ok());
        assert_invalid(validate_remote_endpoint(&format!("{label}a:80")), "64-char label");

        let host_253 = format!("{label}.{label}.{label}.{}", "a".repeat(61));
        assert_eq!(host_253.len(), MAX_HOSTNAME_CHARS);
        // Any 254-char host also overflows the 255-char endpoint, so the
        // hostname bound is checked on its own.
        assert!(check_host(&host_253).is_ok());
        assert!(check_host(&format!("{host_253}a")).is_err(), "254-char host");

        // 253-char host + ":" + 1-char port = 255 chars, the column width.
        let at_limit = format!("{host_253}:1");
        assert_eq!(at_limit.chars().count(), MAX_REMOTE_ENDPOINT_CHARS);
        assert!(validate_remote_endpoint(&at_limit).is_ok());
        let over_limit = format!("{host_253}:01");
        assert_eq!(over_limit.chars().count(), MAX_REMOTE_ENDPOINT_CHARS + 1);
        assert_invalid(validate_remote_endpoint(&over_limit), "256-char endpoint");
    }
}
