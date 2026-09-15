
use std::env;
use std::net::SocketAddr;
use std::time::Duration;
use tonic::transport::Server;
use tonic::server::NamedService;
use tonic_health::ServingStatus;

use grpc_network_gateway::services::gateway::Gateway;
use grpc_network_gateway::services::gateway::proto::gateway_controller_server::GatewayControllerServer;

use sqlx::postgres::PgPoolOptions;

use grpc_network_gateway::network::router::RoutingTable;
use grpc_network_gateway::services::health;

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

    // Lazy: constructing the pool must not require Postgres to be up yet, so a
    // pod scheduled before its database reports not-ready instead of crashing.
    let db_pool = PgPoolOptions::new().max_connections(5).connect_lazy(&database_url)?;

    let routing_table = RoutingTable::new();

    let gateway = Gateway::new(routing_table.clone(), db_pool.clone());

    let (health_reporter, health_service) = tonic_health::server::health_reporter();
    let service_name = <GatewayControllerServer<Gateway> as NamedService>::NAME;

    // Report NotServing until the watcher's first query succeeds, so a probe can
    // never see SERVING before the database has actually answered.
    health_reporter.set_service_status(health::OVERALL, ServingStatus::NotServing).await;
    health_reporter.set_service_status(service_name, ServingStatus::NotServing).await;
    // Hydrates the routing table, then keeps readiness in step with the database.
    health::spawn_readiness_task(
        health_reporter,
        db_pool,
        routing_table,
        service_name,
        Duration::from_secs(5)
    );

    Server::builder()
        .add_service(health_service)
        .add_service(GatewayControllerServer::new(gateway))
        .serve(addr).await?;

    Ok(())
}
