//! End-to-end smoke check against a running gateway.
//!
//!   cargo run --example smoke_client
//!
//! Exercises the reconciliation primitives an operator depends on: create a
//! tunnel, observe it both in the gateway's reported state and in the durable
//! actual state `ListRoutes` returns, update it in place, confirm another owner
//! can neither overwrite nor delete it, confirm a malformed request is refused
//! before any write, delete it and confirm `ListRoutes` no longer lists it, and
//! confirm the delete is idempotent when repeated against an already-absent
//! tunnel.

use grpc_network_gateway::services::gateway::proto::{
    gateway_controller_client::GatewayControllerClient,
    DeleteTunnelRequest,
    ListRoutesRequest,
    Route,
    StatusRequest,
    TunnelRequest,
};
use tonic::transport::Channel;

const LOCAL_IP: &str = "10.0.1.5";
const OWNER: &str = "smoke/smoke-client";
const INTRUDER: &str = "smoke/intruder";

/// Every route `ListRoutes` reports for `LOCAL_IP`. More than one would mean
/// the key is not unique, so callers assert on the whole vector.
async fn listed_routes(
    client: &mut GatewayControllerClient<Channel>
) -> Result<Vec<Route>, tonic::Status> {
    Ok(
        client
            .list_routes(ListRoutesRequest {}).await?
            .into_inner()
            .routes.into_iter()
            .filter(|r| r.local_ip == LOCAL_IP)
            .collect()
    )
}

fn expected_route(tunnel_id: &str, remote_endpoint: &str) -> Route {
    Route {
        local_ip: LOCAL_IP.into(),
        tunnel_id: tunnel_id.into(),
        remote_endpoint: remote_endpoint.into(),
    }
}

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

    let listed = listed_routes(&mut client).await?;
    println!("list    -> {:?}", listed);
    assert_eq!(
        listed,
        vec![expected_route("tun-paris-01", "203.0.113.7:51820")],
        "ListRoutes must return the created tunnel exactly as stored"
    );

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

    let listed = listed_routes(&mut client).await?;
    println!("list    -> after own update: {:?}", listed);
    assert_eq!(
        listed,
        vec![expected_route("tun-paris-02", "203.0.113.8:51820")],
        "ListRoutes must return the updated tunnel"
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

    let listed = listed_routes(&mut client).await?;
    println!("list    -> after foreign writes: {:?}", listed);
    assert_eq!(
        listed,
        vec![expected_route("tun-paris-02", "203.0.113.8:51820")],
        "foreign-owner writes must leave the stored tunnel unchanged"
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

    let listed = listed_routes(&mut client).await?;
    println!("list    -> after invalid create: {:?}", listed);
    assert_eq!(
        listed,
        vec![expected_route("tun-paris-02", "203.0.113.8:51820")],
        "a refused create must leave the stored tunnel unchanged"
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

    let listed = listed_routes(&mut client).await?;
    println!("list    -> after delete: {:?}", listed);
    assert!(listed.is_empty(), "ListRoutes still returns the deleted tunnel");

    // The property a finalizer and a retrying reconcile loop both rely on.
    let second = client.delete_vpn_tunnel(DeleteTunnelRequest {
        local_ip: LOCAL_IP.into(),
        owner: OWNER.into(),
    }).await?.into_inner();
    println!("delete  -> success={} existed={} {}", second.success, second.existed, second.status_message);
    assert!(second.success && !second.existed, "repeat delete must succeed and report existed=false");

    println!(
        "\nOK: create, observe, list, ownership, validation, delete, and idempotent re-delete all behave."
    );
    Ok(())
}
