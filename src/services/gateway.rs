use tonic::{ Request, Response, Status };
use std::net::Ipv4Addr;
use crate::network::router::{ RoutingTable, Route, parse_destination_ip };

pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/grpc.connectivity.rs"));
}

use proto::gateway_controller_server::GatewayController;
use proto::{ TunnelRequest, TunnelResponse, PacketRequest, PacketResponse };

use sqlx::PgPool;

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
        println!("Received request to create tunnel: {}", payload.tunnel_id);

        let local_ip: Ipv4Addr = payload.local_ip
            .parse()
            .map_err(|err| {
                Status::invalid_argument(
                    format!("Invalid local_ip format '{}': {}", payload.local_ip, err)
                )
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
            Status::internal(format!("Database persistence failure: {}", err))
        })?;
    
        let response = TunnelResponse {
            success: true,
            status_message: format!("Tunnel {} successfully created", payload.tunnel_id),
        };

        Ok(Response::new(response))
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
                println!(
                    "Forwarding packet via Tunnel '{}' to remote gateway: {}",
                    route.tunnel_id,
                    route.remote_endpoint
                );
                "FORWARDED".to_string()
            }
            None => {
                println!("No route found for destination IP: {}. Dropping packet.", dest_ip);
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
