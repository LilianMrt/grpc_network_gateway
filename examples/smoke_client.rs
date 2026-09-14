//! End-to-end smoke check against a running gateway.
//!
//!   cargo run --example smoke_client
//!
//! Exercises the reconciliation primitives an operator depends on: create a
//! tunnel, observe it in the gateway's reported state, delete it, and confirm
//! the delete is idempotent when repeated against an already-absent tunnel.

use grpc_network_gateway::services::gateway::proto::{
    gateway_controller_client::GatewayControllerClient,
    DeleteTunnelRequest,
    StatusRequest,
    TunnelRequest,
};

const LOCAL_IP: &str = "10.0.1.5";

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
    }).await?.into_inner();
    println!("create  -> success={} {}", created.success, created.status_message);

    let present = client
        .get_gateway_status(StatusRequest {}).await?
        .into_inner()
        .active_routes.iter()
        .any(|r| r.destination_ip == LOCAL_IP);
    println!("status  -> tunnel present: {}", present);
    assert!(present, "created tunnel missing from gateway status");

    let first = client.delete_vpn_tunnel(DeleteTunnelRequest {
        local_ip: LOCAL_IP.into(),
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
    }).await?.into_inner();
    println!("delete  -> success={} existed={} {}", second.success, second.existed, second.status_message);
    assert!(second.success && !second.existed, "repeat delete must succeed and report existed=false");

    println!("\nOK: create, observe, delete, and idempotent re-delete all behave.");
    Ok(())
}
