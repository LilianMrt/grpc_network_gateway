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

use sqlx::PgPool;
use tracing::{ error, info, warn };

/// Loads persisted routes into the in-memory table, returning how many were
/// applied. Retryable: the routing table is replaced wholesale, so running this
/// again after a failure is safe and converges on the database's contents.
pub async fn hydrate(pool: &PgPool, table: &RoutingTable) -> Result<usize, sqlx::Error> {
    let records = sqlx
        ::query!("SELECT local_ip, tunnel_id, remote_endpoint FROM vpn_routes")
        .fetch_all(pool).await?;

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
    pub db_pool: PgPool
}

impl Gateway {
    pub fn new(routing_table: RoutingTable, db_pool: PgPool) -> Self {
        Self { routing_table, db_pool }
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

        let route_config = Route {
            tunnel_id: payload.tunnel_id.clone(),
            remote_endpoint: payload.remote_endpoint.clone(),
        };

        self.routing_table.add_route(local_ip, route_config).await;

        sqlx::query!(
        "INSERT INTO vpn_routes (local_ip, tunnel_id, remote_endpoint) 
         VALUES ($1, $2, $3)
         ON CONFLICT (local_ip) 
         DO UPDATE SET tunnel_id = EXCLUDED.tunnel_id, remote_endpoint = EXCLUDED.remote_endpoint",
        payload.local_ip,
        payload.tunnel_id,
        payload.remote_endpoint
        )
        .execute(&self.db_pool)
        .await
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

        let result = sqlx
            ::query!("DELETE FROM vpn_routes WHERE local_ip = $1", payload.local_ip)
            .execute(&self.db_pool).await
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

        let existed = result.rows_affected() > 0;
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
