use tonic::{ Request, Response, Status };
use std::net::Ipv4Addr;
use crate::network::router::{ RoutingTable, Route };

pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/grpc.connectivity.rs"));
}

use proto::gateway_controller_server::GatewayController;
use proto::{ TunnelRequest, TunnelResponse, PacketRequest, PacketResponse };

#[derive(Debug)]
pub struct Gateway {
    pub routing_table: RoutingTable,
}

impl Gateway {
    pub fn new(routing_table: RoutingTable) -> Self {
        Self { routing_table }
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
                tonic::Status::invalid_argument(
                    format!("Invalid local_ip format '{}': {}", payload.local_ip, err)
                )
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

    async fn route_packet(
        &self,
        request: Request<PacketRequest>
    ) -> Result<Response<PacketResponse>, Status> {
        // Placeholder for packet simulation logic
        let response = PacketResponse {
            action: "FORWARDED".to_string(),
            bytes_processed: 0,
        };
        Ok(Response::new(response))
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
