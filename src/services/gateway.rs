use tonic::{Request, Response, Status};

pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/grpc.connectivity.rs"));
}

use proto::gateway_controller_server::GatewayController;
use proto::{TunnelRequest, TunnelResponse, PacketRequest, PacketResponse};

#[derive(Debug, Default)]
pub struct Gateway {
    // Future home of our database pool and routing table state!
}

#[tonic::async_trait]
impl GatewayController for Gateway {
    async fn create_vpn_tunnel(
        &self,
        request: Request<TunnelRequest>,
    ) -> Result<Response<TunnelResponse>, Status> {
        let payload = request.into_inner();
        println!("Received request to create tunnel: {}", payload.tunnel_id);

        // Your architecture task will be to parse/validate here!

        let response = TunnelResponse {
            success: true,
            status_message: format!("Tunnel {} successfully created", payload.tunnel_id),
        };
        Ok(Response::new(response))
    }

    async fn route_packet(
        &self,
        request: Request<PacketRequest>,
    ) -> Result<Response<PacketResponse>, Status> {
        // Placeholder for packet simulation logic
        let response = PacketResponse {
            action: "FORWARDED".to_string(),
            bytes_processed: 0,
        };
        Ok(Response::new(response))
    }
}