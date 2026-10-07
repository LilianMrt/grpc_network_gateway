//! End-to-end smoke check against a running gateway.
//!
//!   cargo run --example smoke_client
//!
//! Exercises the reconciliation primitives an operator depends on: create a
//! tunnel, observe it in the gateway's reported state, confirm another owner
//! can neither overwrite nor delete it, confirm a malformed request is refused
//! before any write, delete it, and confirm the delete is
//! idempotent when repeated against an already-absent tunnel.

use grpc_network_gateway::services::gateway::proto::{
    gateway_controller_client::GatewayControllerClient,
    DeleteTunnelRequest,
    StatusRequest,
    TunnelRequest,
};

const LOCAL_IP: &str = "10.0.1.5";
const OWNER: &str = "smoke/smoke-client";
const INTRUDER: &str = "smoke/intruder";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = std::env
        ::var("GATEWAY_ADDR")
        .unwrap_or_else(|_| "http://127.0.0.1:50051".to_string());
    let mut client = GatewayControllerClient::connect(addr).await?;

    let created = client.create_vpn_tunnel(TunnelRequest {
        tunnel_id: "tun-paris-01".into(),
        local_ip: LOCAL_IP.into(),
        remote_endpoint: "203.0.113.7:51820".into(),
        owner: OWNER.into(),
    }).await?.into_inner();
    println!("create  -> success={} {}", created.success, created.status_message);

    let present = client
        .get_gateway_status(StatusRequest {}).await?
        .into_inner()
        .active_routes.iter()
        .any(|r| r.destination_ip == LOCAL_IP);
    println!("status  -> tunnel present: {}", present);
    assert!(present, "created tunnel missing from gateway status");

    // The owner re-declaring its own route updates it in place.
    let updated = client.create_vpn_tunnel(TunnelRequest {
        tunnel_id: "tun-paris-02".into(),
        local_ip: LOCAL_IP.into(),
        remote_endpoint: "203.0.113.8:51820".into(),
        owner: OWNER.into(),
    }).await?.into_inner();
    println!("create  -> own update: success={} {}", updated.success, updated.status_message);

    let route = client
        .get_gateway_status(StatusRequest {}).await?
        .into_inner()
        .active_routes.into_iter()
        .find(|r| r.destination_ip == LOCAL_IP);
    println!("status  -> tunnel after own update: {:?}", route.as_ref().map(|r| &r.tunnel_id));
    assert_eq!(
        route.map(|r| r.tunnel_id),
        Some("tun-paris-02".to_string()),
        "the owner's own create must update the tunnel"
    );

    // Ownership (AD-13): a resource in another namespace that declares the
    // same local_ip must not take the route over or tear it down.
    let foreign_create = client.create_vpn_tunnel(TunnelRequest {
        tunnel_id: "tun-intruder".into(),
        local_ip: LOCAL_IP.into(),
        remote_endpoint: "198.51.100.66:51820".into(),
        owner: INTRUDER.into(),
    }).await;
    let code = foreign_create.as_ref().err().map(|status| status.code());
    println!("create  -> foreign owner: {:?}", code);
    assert_eq!(
        code,
        Some(tonic::Code::FailedPrecondition),
        "a foreign-owner create must be refused with FAILED_PRECONDITION"
    );

    let foreign_delete = client.delete_vpn_tunnel(DeleteTunnelRequest {
        local_ip: LOCAL_IP.into(),
        owner: INTRUDER.into(),
    }).await?.into_inner();
    println!(
        "delete  -> foreign owner: success={} existed={} {}",
        foreign_delete.success, foreign_delete.existed, foreign_delete.status_message
    );
    assert!(
        foreign_delete.success && !foreign_delete.existed,
        "a foreign-owner delete must succeed and report existed=false"
    );

    let route = client
        .get_gateway_status(StatusRequest {}).await?
        .into_inner()
        .active_routes.into_iter()
        .find(|r| r.destination_ip == LOCAL_IP);
    println!("status  -> tunnel after foreign writes: {:?}", route.as_ref().map(|r| &r.tunnel_id));
    assert_eq!(
        route.map(|r| r.tunnel_id),
        Some("tun-paris-02".to_string()),
        "foreign-owner writes must leave the tunnel unchanged"
    );

    // Validation runs before any write: a malformed endpoint is refused as a
    // permanent INVALID_ARGUMENT, not stored and not retried.
    let invalid_create = client.create_vpn_tunnel(TunnelRequest {
        tunnel_id: "tun-paris-03".into(),
        local_ip: LOCAL_IP.into(),
        remote_endpoint: "no-port".into(),
        owner: OWNER.into(),
    }).await;
    let code = invalid_create.as_ref().err().map(|status| status.code());
    println!("create  -> invalid endpoint: {:?}", code);
    assert_eq!(
        code,
        Some(tonic::Code::InvalidArgument),
        "a malformed remote_endpoint must be refused with INVALID_ARGUMENT"
    );

    let first = client.delete_vpn_tunnel(DeleteTunnelRequest {
        local_ip: LOCAL_IP.into(),
        owner: OWNER.into(),
    }).await?.into_inner();
    println!("delete  -> success={} existed={} {}", first.success, first.existed, first.status_message);
    assert!(first.success && first.existed, "first delete should report the tunnel existed");

    let still_present = client
        .get_gateway_status(StatusRequest {}).await?
        .into_inner()
        .active_routes.iter()
        .any(|r| r.destination_ip == LOCAL_IP);
    println!("status  -> tunnel present: {}", still_present);
    assert!(!still_present, "tunnel still listed after delete");

    // The property a finalizer and a retrying reconcile loop both rely on.
    let second = client.delete_vpn_tunnel(DeleteTunnelRequest {
        local_ip: LOCAL_IP.into(),
        owner: OWNER.into(),
    }).await?.into_inner();
    println!("delete  -> success={} existed={} {}", second.success, second.existed, second.status_message);
    assert!(second.success && !second.existed, "repeat delete must succeed and report existed=false");

    println!("\nOK: create, observe, ownership, validation, delete, and idempotent re-delete all behave.");
    Ok(())
}
