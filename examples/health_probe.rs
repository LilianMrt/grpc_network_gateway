//! Queries the gateway's gRPC health service once and prints the result.
//!
//!   cargo run --example health_probe
//!
//! Mirrors what Kubernetes does with a `grpc:` probe: it asks for the overall
//! service (the empty name) and treats SERVING as pass, anything else as fail.
//! Exits non-zero when not serving, so it works in a shell test too.

use tonic::transport::Channel;
use tonic_health::pb::HealthCheckRequest;
use tonic_health::pb::health_client::HealthClient;
use tonic_health::pb::health_check_response::ServingStatus;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = std::env
        ::var("GATEWAY_ADDR")
        .unwrap_or_else(|_| "http://127.0.0.1:50051".to_string());
    let service = std::env::var("HEALTH_SERVICE").unwrap_or_default();

    let channel = Channel::from_shared(addr)?.connect().await?;
    let mut client = HealthClient::new(channel);
    let response = client.check(HealthCheckRequest { service: service.clone() }).await?.into_inner();

    let status = ServingStatus::try_from(response.status).unwrap_or(ServingStatus::Unknown);
    let label = if service.is_empty() { "<overall>" } else { &service };
    println!("health({}) = {:?}", label, status);

    if status == ServingStatus::Serving {
        Ok(())
    } else {
        std::process::exit(1);
    }
}
