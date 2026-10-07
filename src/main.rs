
use std::env;
use std::net::SocketAddr;
use std::time::Duration;
use tonic::transport::Server;
use tonic::server::NamedService;
use tonic_health::ServingStatus;
use tracing::info;

use grpc_network_gateway::services::gateway::Gateway;
use grpc_network_gateway::services::gateway::proto::gateway_controller_server::GatewayControllerServer;

use grpc_network_gateway::logging;
use grpc_network_gateway::network::router::RoutingTable;
use grpc_network_gateway::services::health;
use grpc_network_gateway::services::health::{ LIVENESS, OVERALL };
use grpc_network_gateway::store::Store;

/// How long a call waits for a database connection before failing
/// `UNAVAILABLE`, instead of the 30s it would wait by default. Story 3.1's
/// default per-call deadline on the operator must exceed this, so that a
/// database outage reaches it as a retryable `UNAVAILABLE` and not as its own
/// `DEADLINE_EXCEEDED` — though only waiting for and opening a connection is
/// bounded; a query that hangs after acquire is not. `proto/gateway.proto`
/// restates the 5s value: change both together.
const POOL_ACQUIRE_TIMEOUT: Duration = Duration::from_secs(5);

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load .env for local runs. In a container the environment is already set,
    // so a missing file is not an error.
    let _ = dotenvy::dotenv();
    // Right after .env, so a RUST_LOG set there applies, and before anything logs.
    logging::init();

    let addr: SocketAddr = env
        ::var("BIND_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:50051".to_string())
        .parse()?;

    let database_url = env
        ::var("DATABASE_URL")
        .map_err(|_| "DATABASE_URL must be set (see .env.example)")?;

    info!(%addr, "gRPC Control Plane listening");

    // Lazy: constructing the pool must not require Postgres to be up yet, so a
    // pod scheduled before its database reports not-ready instead of crashing.
    let store = Store::connect_lazy(&database_url, POOL_ACQUIRE_TIMEOUT)?;

    let routing_table = RoutingTable::new();

    let gateway = Gateway::new(routing_table.clone(), store.clone());

    let (health_reporter, health_service) = tonic_health::server::health_reporter();
    let service_name = <GatewayControllerServer<Gateway> as NamedService>::NAME;

    // Readiness starts NotServing so a probe can never observe SERVING before the
    // database has actually answered. Liveness is SERVING from here on: the
    // process is up, and a database outage must not restart it.
    health_reporter.set_service_status(OVERALL, ServingStatus::NotServing).await;
    health_reporter.set_service_status(service_name, ServingStatus::NotServing).await;
    health_reporter.set_service_status(LIVENESS, ServingStatus::Serving).await;
    // Hydrates the routing table, then keeps readiness in step with the database.
    health::spawn_readiness_task(
        health_reporter,
        store,
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
