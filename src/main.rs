mod services;
mod network;

use std::net::SocketAddr;
use tonic::transport::Server;

use services::gateway::Gateway;
use services::gateway::proto::gateway_controller_server::GatewayControllerServer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr: SocketAddr = "0.0.0.0:50051".parse()?;
    println!("gRPC Control Plane listening on {}", addr);

    let routing_table = crate::network::router::RoutingTable::new();
    let gateway = Gateway::new(routing_table);

    Server::builder() 
        .add_service(GatewayControllerServer::new(gateway))
        .serve(addr)
        .await?;

    Ok(())
}
