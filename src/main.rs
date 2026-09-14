
use std::collections::HashMap;
use std::env;
use std::net::{ Ipv4Addr, SocketAddr };
use tonic::transport::Server;

use grpc_network_gateway::services::gateway::Gateway;
use grpc_network_gateway::services::gateway::proto::gateway_controller_server::GatewayControllerServer;

use sqlx::postgres::PgPoolOptions;

use grpc_network_gateway::network::router::{ Route, RoutingTable };

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load .env for local runs. In a container the environment is already set,
    // so a missing file is not an error.
    let _ = dotenvy::dotenv();

    let addr: SocketAddr = env
        ::var("BIND_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:50051".to_string())
        .parse()?;

    let database_url = env
        ::var("DATABASE_URL")
        .map_err(|_| "DATABASE_URL must be set (see .env.example)")?;

    println!("gRPC Control Plane listening on {}", addr);

    let db_pool = PgPoolOptions::new().max_connections(5).connect(&database_url).await?;

    let routing_table = RoutingTable::new();
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
