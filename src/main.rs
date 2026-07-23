mod services;
mod network;

use std::collections::HashMap;
use std::net::{ Ipv4Addr, SocketAddr };
use tonic::transport::Server;

use services::gateway::Gateway;
use services::gateway::proto::gateway_controller_server::GatewayControllerServer;

use sqlx::postgres::PgPoolOptions;

use crate::network::router::Route;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr: SocketAddr = "0.0.0.0:50051".parse()?;
    println!("gRPC Control Plane listening on {}", addr);

    let database_url = "postgres://netforge_user:netforge_password@localhost:5432/netforge_db";

    let db_pool = PgPoolOptions::new().max_connections(5).connect(database_url).await?;

    let routing_table = crate::network::router::RoutingTable::new();
    //hydrate
    let records = sqlx
        ::query!("SELECT local_ip, tunnel_id, remote_endpoint FROM vpn_routes")
        .fetch_all(&db_pool).await?;

    let mut initial_routes = HashMap::new();
    for row in records {
        if let Ok(ip) = row.local_ip.parse::<Ipv4Addr>() {
            initial_routes.insert(ip, Route {
                tunnel_id: row.tunnel_id,
                remote_endpoint: row.remote_endpoint,
            });
        }
    }
    routing_table.load_routes(initial_routes).await;

    let gateway = Gateway::new(routing_table, db_pool);

    Server::builder().add_service(GatewayControllerServer::new(gateway)).serve(addr).await?;

    Ok(())
}
