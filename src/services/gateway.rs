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

use crate::store::{ self, Store };
use tracing::{ error, info, warn };

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
        info!(
            local_ip = %payload.local_ip,
            tunnel_id = %payload.tunnel_id,
            "received request to create tunnel"
        );

        let local_ip: Ipv4Addr = payload.local_ip
            .parse()
            .map_err(|err| {
                let status = Status::invalid_argument(
                    format!("Invalid local_ip format '{}': {}", payload.local_ip, err)
                );
                warn!(
                    local_ip = %payload.local_ip,
                    code = ?status.code(),
                    error = %err,
                    "create rejected: invalid local_ip"
                );
                status
            })?;

        // Store first, cache second: a failed write must leave the routing
        // table untouched, or packets would route through a tunnel the client
        // was told failed.
        self.store
            .upsert_route(&payload.local_ip, &payload.tunnel_id, &payload.remote_endpoint).await
            .map_err(|err| {
                let status = Status::internal(format!("Database persistence failure: {}", err));
                error!(
                    %local_ip,
                    code = ?status.code(),
                    error = %err,
                    "create failed: database persistence failure"
                );
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
                    local_ip = %payload.local_ip,
                    code = ?status.code(),
                    error = %err,
                    "delete rejected: invalid local_ip"
                );
                status
            })?;

        // Store first, cache second: a failed delete must leave the route
        // cached, because the row still exists and dropping it from memory would
        // stop this pod forwarding a tunnel the database still holds.
        let existed = self.store
            .delete_route(&payload.local_ip).await
            .map_err(|err| {
                let status = Status::internal(format!("Database delete failure: {}", err));
                error!(
                    %local_ip,
                    code = ?status.code(),
                    error = %err,
                    "delete failed: database delete failure"
                );
                status
            })?;

        self.routing_table.remove_route(&local_ip).await;

        let status_message = if existed {
            format!("Tunnel for {} deleted", local_ip)
        } else {
            format!("No tunnel for {}, nothing to delete", local_ip)
        };
        info!(%local_ip, existed, "tunnel delete handled");

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
}

#[cfg(test)]
mod tests {
    //! Store-first ordering, proved against a store that cannot reach Postgres:
    //! a failed write must leave the routing table exactly as it was.

    use super::*;
    use tonic::Code;

    const IP: &str = "10.0.0.5";

    fn ip() -> Ipv4Addr {
        IP.parse().unwrap()
    }

    fn gateway_with_db_down() -> Gateway {
        Gateway::new(RoutingTable::new(), Store::unreachable())
    }

    fn create_request(tunnel_id: &str, remote_endpoint: &str) -> Request<TunnelRequest> {
        Request::new(TunnelRequest {
            local_ip: IP.to_string(),
            tunnel_id: tunnel_id.to_string(),
            remote_endpoint: remote_endpoint.to_string(),
        })
    }

    async fn status_route(gateway: &Gateway) -> Option<proto::RouteDetails> {
        gateway
            .get_gateway_status(Request::new(proto::StatusRequest {})).await
            .expect("GetGatewayStatus reads only the cache and cannot fail")
            .into_inner()
            .active_routes.into_iter()
            .find(|route| route.destination_ip == IP)
    }

    fn assert_internal(status: &Status, prefix: &str) {
        assert_eq!(status.code(), Code::Internal, "unexpected status: {status:?}");
        assert!(
            status.message().starts_with(prefix),
            "message {:?} does not start with {prefix:?}",
            status.message()
        );
    }

    #[tokio::test]
    async fn create_with_db_down_leaves_empty_table_empty() {
        let gateway = gateway_with_db_down();

        let status = gateway
            .create_vpn_tunnel(create_request("tun-new", "203.0.113.9:51820")).await
            .expect_err("create must fail when the database is down");

        assert_internal(&status, "Database persistence failure");
        assert!(gateway.routing_table.lookup_route(&ip()).await.is_none());
        assert!(gateway.routing_table.get_all_routes().await.is_empty());
        assert!(status_route(&gateway).await.is_none());
    }

    #[tokio::test]
    async fn create_with_db_down_keeps_cached_route() {
        let gateway = gateway_with_db_down();
        gateway.routing_table.add_route(ip(), Route {
            tunnel_id: "tun-old".to_string(),
            remote_endpoint: "198.51.100.1:51820".to_string(),
        }).await;

        let status = gateway
            .create_vpn_tunnel(create_request("tun-new", "203.0.113.9:51820")).await
            .expect_err("create must fail when the database is down");

        assert_internal(&status, "Database persistence failure");
        let cached = gateway.routing_table.lookup_route(&ip()).await.expect("route still cached");
        assert_eq!(cached.tunnel_id, "tun-old");
        assert_eq!(cached.remote_endpoint, "198.51.100.1:51820");
        let reported = status_route(&gateway).await.expect("route still reported");
        assert_eq!(reported.tunnel_id, "tun-old");
        assert_eq!(reported.remote_endpoint, "198.51.100.1:51820");
    }

    #[tokio::test]
    async fn delete_with_db_down_keeps_cached_route() {
        let gateway = gateway_with_db_down();
        gateway.routing_table.add_route(ip(), Route {
            tunnel_id: "tun-old".to_string(),
            remote_endpoint: "198.51.100.1:51820".to_string(),
        }).await;

        let status = gateway
            .delete_vpn_tunnel(Request::new(DeleteTunnelRequest { local_ip: IP.to_string() })).await
            .expect_err("delete must fail when the database is down");

        assert_internal(&status, "Database delete failure");
        let cached = gateway.routing_table.lookup_route(&ip()).await.expect("route still cached");
        assert_eq!(cached.tunnel_id, "tun-old");
        assert_eq!(cached.remote_endpoint, "198.51.100.1:51820");
        let reported = status_route(&gateway).await.expect("route still reported");
        assert_eq!(reported.tunnel_id, "tun-old");
        assert_eq!(reported.remote_endpoint, "198.51.100.1:51820");
    }
}
