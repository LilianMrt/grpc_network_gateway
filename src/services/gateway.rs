use tonic::{ Request, Response, Status };
use std::collections::HashMap;
use std::net::Ipv4Addr;
use crate::network::router::{ RoutingTable, Route, parse_destination_ip };

pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/grpc.connectivity.rs"));
}

use proto::gateway_controller_server::GatewayController;
use proto::{
    TunnelRequest,
    TunnelResponse,
    PacketRequest,
    PacketResponse,
    DeleteTunnelRequest,
    DeleteTunnelResponse,
};

use crate::services::validation::{ check_owner, validate_remote_endpoint, validate_tunnel_id };
use crate::store::{ self, Store };
use tonic::Code;
use tracing::{ debug, error, info, warn };

/// Loads persisted routes into the in-memory table, returning how many were
/// applied. Retryable: the routing table is replaced wholesale, so running this
/// again after a failure is safe and converges on the database's contents.
pub async fn hydrate(store: &Store, table: &RoutingTable) -> Result<usize, store::Error> {
    let records = store.list_routes().await?;

    let mut routes = HashMap::new();
    for row in records {
        match row.local_ip.parse::<Ipv4Addr>() {
            Ok(ip) => {
                routes.insert(ip, Route {
                    tunnel_id: row.tunnel_id,
                    remote_endpoint: row.remote_endpoint,
                });
            }
            Err(err) => {
                warn!(
                    local_ip = %row.local_ip,
                    tunnel_id = %row.tunnel_id,
                    error = %err,
                    "skipping persisted route with unparseable local_ip"
                );
            }
        }
    }

    let count = routes.len();
    table.load_routes(routes).await;
    Ok(count)
}

/// Logs a write that failed with `status`. A caller's mistake (bad input, a
/// route it does not own) is a warning; anything else is an error.
fn log_failed_write(local_ip: Ipv4Addr, owner: &str, status: &Status, message: &'static str) {
    match status.code() {
        Code::InvalidArgument | Code::FailedPrecondition => {
            warn!(%local_ip, ?owner, code = ?status.code(), error = %status.message(), "{}", message);
        }
        _ => {
            error!(%local_ip, ?owner, code = ?status.code(), error = %status.message(), "{}", message);
        }
    }
}

/// Stored rows as `ListRoutes` sends them: verbatim, in the same order. A row
/// whose local_ip does not parse is still actual state. `proto::Route` is
/// spelled out because `Route` is the cache's type.
fn to_wire(rows: Vec<store::StoredRoute>) -> Vec<proto::Route> {
    rows.into_iter()
        .map(|row| proto::Route {
            local_ip: row.local_ip,
            tunnel_id: row.tunnel_id,
            remote_endpoint: row.remote_endpoint,
        })
        .collect()
}

#[derive(Debug)]
pub struct Gateway {
    pub routing_table: RoutingTable,
    pub store: Store,
}

impl Gateway {
    pub fn new(routing_table: RoutingTable, store: Store) -> Self {
        Self { routing_table, store }
    }
}

#[tonic::async_trait]
impl GatewayController for Gateway {
    async fn create_vpn_tunnel(
        &self,
        request: Request<TunnelRequest>
    ) -> Result<Response<TunnelResponse>, Status> {
        let payload = request.into_inner();
        // Debug-formatted: nothing is validated yet, and `?` escapes a CR/LF
        // that would otherwise forge a log line.
        info!(
            local_ip = ?payload.local_ip,
            tunnel_id = ?payload.tunnel_id,
            owner = ?payload.owner,
            "received request to create tunnel"
        );

        let local_ip: Ipv4Addr = payload.local_ip
            .parse()
            .map_err(|err| {
                let status = Status::invalid_argument(
                    format!("Invalid local_ip format '{}': {}", payload.local_ip, err)
                );
                warn!(
                    local_ip = ?payload.local_ip,
                    code = ?status.code(),
                    error = %err,
                    "create rejected: invalid local_ip"
                );
                status
            })?;
        // Every check runs before the store is called, in this order. A value
        // past here fits its column, so the database's own checks are a backstop.
        check_owner(&payload.owner)
            .and_then(|()| validate_tunnel_id(&payload.tunnel_id))
            .and_then(|()| validate_remote_endpoint(&payload.remote_endpoint))
            .map_err(|status| {
                warn!(%local_ip, code = ?status.code(), error = %status.message(), "create rejected");
                status
            })?;

        // Store first, cache second: a failed write must leave the routing
        // table untouched, or packets would route through a tunnel the client
        // was told failed. The store decides the code, including
        // FAILED_PRECONDITION for a route another owner holds.
        self.store
            .upsert_route(
                &payload.local_ip,
                &payload.tunnel_id,
                &payload.remote_endpoint,
                &payload.owner
            ).await
            .map_err(|err| {
                let status = Status::from(err);
                log_failed_write(local_ip, &payload.owner, &status, "create failed");
                status
            })?;

        let route_config = Route {
            tunnel_id: payload.tunnel_id.clone(),
            remote_endpoint: payload.remote_endpoint.clone(),
        };
        self.routing_table.add_route(local_ip, route_config).await;

        let response = TunnelResponse {
            success: true,
            status_message: format!("Tunnel {} successfully created", payload.tunnel_id),
        };

        Ok(Response::new(response))
    }

    async fn delete_vpn_tunnel(
        &self,
        request: Request<DeleteTunnelRequest>
    ) -> Result<Response<DeleteTunnelResponse>, Status> {
        let payload = request.into_inner();

        let local_ip: Ipv4Addr = payload.local_ip
            .parse()
            .map_err(|err| {
                let status = Status::invalid_argument(
                    format!("Invalid local_ip format '{}': {}", payload.local_ip, err)
                );
                warn!(
                    local_ip = ?payload.local_ip,
                    code = ?status.code(),
                    error = %err,
                    "delete rejected: invalid local_ip"
                );
                status
            })?;
        check_owner(&payload.owner).map_err(|status| {
            warn!(%local_ip, code = ?status.code(), error = %status.message(), "delete rejected");
            status
        })?;

        // Store first, cache second: a failed delete must leave the route
        // cached, because the row still exists and dropping it from memory would
        // stop this pod forwarding a tunnel the database still holds.
        let existed = self.store
            .delete_route(&payload.local_ip, &payload.owner).await
            .map_err(|err| {
                let status = Status::from(err);
                log_failed_write(local_ip, &payload.owner, &status, "delete failed");
                status
            })?;

        // Only a removed row clears the cache: when nothing was deleted the
        // route may belong to another owner and is still live.
        if existed {
            self.routing_table.remove_route(&local_ip).await;
        }

        let status_message = if existed {
            format!("Tunnel for {} deleted", local_ip)
        } else {
            format!("No tunnel for {} owned by {}, nothing to delete", local_ip, payload.owner)
        };
        info!(%local_ip, owner = ?payload.owner, existed, "tunnel delete handled");

        // Deleting an absent tunnel is success, not an error: a reconcile loop
        // converges toward a desired state and must be safe to run repeatedly.
        Ok(Response::new(DeleteTunnelResponse { success: true, existed, status_message }))
    }

    async fn route_packet(
        &self,
        request: Request<PacketRequest>
    ) -> Result<Response<PacketResponse>, Status> {
        let packet_bytes = &request.into_inner().payload;

        let dest_ip = match parse_destination_ip(packet_bytes) {
            Ok(ip) => ip,
            Err(_) => {
                return Ok(
                    Response::new(PacketResponse {
                        action: "DROPPED (MALFORMED_HEADER)".to_string(),
                        bytes_processed: packet_bytes.len() as u32,
                    })
                );
            }
        };

        let search_result = self.routing_table.lookup_route(&dest_ip).await;

        let action = match search_result {
            Some(route) => {
                info!(
                    %dest_ip,
                    tunnel_id = %route.tunnel_id,
                    remote_endpoint = %route.remote_endpoint,
                    "forwarding packet via tunnel to remote gateway"
                );
                "FORWARDED".to_string()
            }
            None => {
                info!(%dest_ip, "no route found for destination, dropping packet");
                "DROPPED (NO_ROUTE)".to_string()
            }
        };

        Ok(
            Response::new(PacketResponse {
                action,
                bytes_processed: packet_bytes.len() as u32,
            })
        )
    }

    async fn get_gateway_status(
        &self,
        _request: Request<proto::StatusRequest>
    ) -> Result<Response<proto::StatusResponse>, Status> {
        let routes_snapshot = self.routing_table.get_all_routes().await;

        let mut active_routes = Vec::new();
        for (ip, route) in routes_snapshot {
            active_routes.push(proto::RouteDetails {
                destination_ip: ip.to_string(),
                tunnel_id: route.tunnel_id,
                remote_endpoint: route.remote_endpoint,
            });
        }

        Ok(Response::new(proto::StatusResponse { active_routes }))
    }

    /// Actual state, read from the database alone. The routing table is never
    /// consulted, so the answer does not depend on which pod serves the call or
    /// whether it hydrated. A store failure is an error, never an empty list:
    /// a reconciler would read an empty list as every route having vanished.
    async fn list_routes(
        &self,
        _request: Request<proto::ListRoutesRequest>
    ) -> Result<Response<proto::ListRoutesResponse>, Status> {
        let rows = self.store.list_routes().await.map_err(|err| {
            let status = Status::from(err);
            match status.code() {
                Code::Unavailable | Code::Internal => {
                    error!(code = ?status.code(), error = %status.message(), "list routes failed");
                }
                _ => {
                    warn!(code = ?status.code(), error = %status.message(), "list routes failed");
                }
            }
            status
        })?;

        let routes = to_wire(rows);

        // Debug, not info: a reconciler polls this on every pass.
        debug!(count = routes.len(), "listed routes");
        Ok(Response::new(proto::ListRoutesResponse { routes }))
    }
}

#[cfg(test)]
mod tests {
    //! Store-first ordering, proved against a store that cannot reach Postgres:
    //! a failed write must leave the routing table exactly as it was. The same
    //! store proves input validation runs before any write: a request that
    //! reached the store would fail `Unavailable`, not `InvalidArgument`.

    use super::*;
    use crate::services::validation;

    const IP: &str = "10.0.0.5";
    const OWNER: &str = "default/tunnel-a";

    fn ip() -> Ipv4Addr {
        IP.parse().unwrap()
    }

    fn gateway_with_db_down() -> Gateway {
        Gateway::new(RoutingTable::new(), Store::unreachable())
    }

    fn create_request(tunnel_id: &str, remote_endpoint: &str) -> Request<TunnelRequest> {
        create_request_owned_by(tunnel_id, remote_endpoint, OWNER)
    }

    fn create_request_owned_by(
        tunnel_id: &str,
        remote_endpoint: &str,
        owner: &str
    ) -> Request<TunnelRequest> {
        Request::new(TunnelRequest {
            local_ip: IP.to_string(),
            tunnel_id: tunnel_id.to_string(),
            remote_endpoint: remote_endpoint.to_string(),
            owner: owner.to_string(),
        })
    }

    fn delete_request(owner: &str) -> Request<DeleteTunnelRequest> {
        Request::new(DeleteTunnelRequest { local_ip: IP.to_string(), owner: owner.to_string() })
    }

    async fn cache_old_route(gateway: &Gateway) {
        gateway.routing_table.add_route(ip(), Route {
            tunnel_id: "tun-old".to_string(),
            remote_endpoint: "198.51.100.1:51820".to_string(),
        }).await;
    }

    async fn assert_old_route_kept(gateway: &Gateway) {
        let cached = gateway.routing_table.lookup_route(&ip()).await.expect("route still cached");
        assert_eq!(cached.tunnel_id, "tun-old");
        assert_eq!(cached.remote_endpoint, "198.51.100.1:51820");
        let reported = status_route(gateway).await.expect("route still reported");
        assert_eq!(reported.tunnel_id, "tun-old");
        assert_eq!(reported.remote_endpoint, "198.51.100.1:51820");
        assert_eq!(gateway.routing_table.get_all_routes().await.len(), 1);
    }

    async fn status_route(gateway: &Gateway) -> Option<proto::RouteDetails> {
        gateway
            .get_gateway_status(Request::new(proto::StatusRequest {})).await
            .expect("GetGatewayStatus reads only the cache and cannot fail")
            .into_inner()
            .active_routes.into_iter()
            .find(|route| route.destination_ip == IP)
    }

    fn assert_unavailable(status: &Status) {
        assert_eq!(status.code(), Code::Unavailable, "unexpected status: {status:?}");
        assert!(
            status.message().starts_with("database unavailable"),
            "message {:?} does not start with \"database unavailable\"",
            status.message()
        );
    }

    #[tokio::test]
    async fn create_with_db_down_leaves_empty_table_empty() {
        let gateway = gateway_with_db_down();

        let status = gateway
            .create_vpn_tunnel(create_request("tun-new", "203.0.113.9:51820")).await
            .expect_err("create must fail when the database is down");

        assert_unavailable(&status);
        assert!(gateway.routing_table.lookup_route(&ip()).await.is_none());
        assert!(gateway.routing_table.get_all_routes().await.is_empty());
        assert!(status_route(&gateway).await.is_none());
    }

    #[tokio::test]
    async fn create_with_db_down_keeps_cached_route() {
        let gateway = gateway_with_db_down();
        cache_old_route(&gateway).await;

        let status = gateway
            .create_vpn_tunnel(create_request("tun-new", "203.0.113.9:51820")).await
            .expect_err("create must fail when the database is down");

        assert_unavailable(&status);
        assert_old_route_kept(&gateway).await;
    }

    #[tokio::test]
    async fn delete_with_db_down_keeps_cached_route() {
        let gateway = gateway_with_db_down();
        cache_old_route(&gateway).await;

        let status = gateway
            .delete_vpn_tunnel(delete_request(OWNER)).await
            .expect_err("delete must fail when the database is down");

        assert_unavailable(&status);
        assert_old_route_kept(&gateway).await;
    }

    #[tokio::test]
    async fn list_routes_with_db_down_is_unavailable_and_ignores_cache() {
        let gateway = gateway_with_db_down();
        cache_old_route(&gateway).await;

        let status = gateway
            .list_routes(Request::new(proto::ListRoutesRequest {})).await
            .expect_err("ListRoutes must fail when the database is down, not serve the cache");

        assert_unavailable(&status);
        assert_old_route_kept(&gateway).await;
    }

    #[test]
    fn to_wire_copies_rows_verbatim_in_order() {
        let row = |local_ip: &str, tunnel_id: &str, remote_endpoint: &str| store::StoredRoute {
            local_ip: local_ip.to_string(),
            tunnel_id: tunnel_id.to_string(),
            remote_endpoint: remote_endpoint.to_string(),
        };
        let rows = vec![
            row("not-an-ip", "tun-bad", "bad.example:1"),
            row("010.0.1.5", "tun-zero", "198.51.100.2:080"),
            row("10.0.1.5", "tun-canonical", "203.0.113.7:51820")
        ];

        let wire = to_wire(rows.clone());

        assert_eq!(wire.len(), rows.len());
        for (sent, stored) in wire.iter().zip(&rows) {
            assert_eq!(sent.local_ip, stored.local_ip);
            assert_eq!(sent.tunnel_id, stored.tunnel_id);
            assert_eq!(sent.remote_endpoint, stored.remote_endpoint);
        }
    }

    #[tokio::test]
    async fn create_with_empty_owner_is_rejected_before_any_write() {
        let gateway = gateway_with_db_down();
        cache_old_route(&gateway).await;

        let status = gateway
            .create_vpn_tunnel(create_request_owned_by("tun-new", "203.0.113.9:51820", "")).await
            .expect_err("create with an empty owner must be rejected");

        assert_eq!(status.code(), Code::InvalidArgument, "unexpected status: {status:?}");
        assert_old_route_kept(&gateway).await;
    }

    #[tokio::test]
    async fn delete_with_empty_owner_is_rejected_before_any_write() {
        let gateway = gateway_with_db_down();
        cache_old_route(&gateway).await;

        let status = gateway
            .delete_vpn_tunnel(delete_request("")).await
            .expect_err("delete with an empty owner must be rejected");

        assert_eq!(status.code(), Code::InvalidArgument, "unexpected status: {status:?}");
        assert_old_route_kept(&gateway).await;
    }

    #[tokio::test]
    async fn create_with_overlong_tunnel_id_is_rejected_before_any_write() {
        let gateway = gateway_with_db_down();
        cache_old_route(&gateway).await;

        let tunnel_id = "t".repeat(validation::MAX_TUNNEL_ID_CHARS + 1);
        let status = gateway
            .create_vpn_tunnel(create_request(&tunnel_id, "203.0.113.9:51820")).await
            .expect_err("create with an overlong tunnel_id must be rejected");

        assert_eq!(status.code(), Code::InvalidArgument, "unexpected status: {status:?}");
        assert_old_route_kept(&gateway).await;
    }

    #[tokio::test]
    async fn create_with_bad_endpoint_is_rejected_before_any_write() {
        let gateway = gateway_with_db_down();
        cache_old_route(&gateway).await;

        let status = gateway
            .create_vpn_tunnel(create_request("tun-new", "no-port")).await
            .expect_err("create with a malformed remote_endpoint must be rejected");

        assert_eq!(status.code(), Code::InvalidArgument, "unexpected status: {status:?}");
        assert_old_route_kept(&gateway).await;
    }

    #[tokio::test]
    async fn create_reports_the_first_failing_check_in_order() {
        let gateway = gateway_with_db_down();
        let tunnel_id = "t".repeat(validation::MAX_TUNNEL_ID_CHARS + 1);

        let status = gateway
            .create_vpn_tunnel(create_request_owned_by(&tunnel_id, "no-port", "")).await
            .expect_err("every field is invalid");
        assert_eq!(status.code(), Code::InvalidArgument, "unexpected status: {status:?}");
        assert!(status.message().starts_with("owner must not be empty"), "{status:?}");

        let status = gateway
            .create_vpn_tunnel(create_request(&tunnel_id, "no-port")).await
            .expect_err("tunnel_id and remote_endpoint are invalid");
        assert_eq!(status.code(), Code::InvalidArgument, "unexpected status: {status:?}");
        assert!(status.message().starts_with("tunnel_id must be"), "{status:?}");
    }
}
