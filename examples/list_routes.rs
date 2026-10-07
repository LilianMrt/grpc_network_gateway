//! Prints the gateway's actual state: every route persisted in the database.
//!
//!   cargo run --example list_routes
//!
//! Calls `ListRoutes`, which reads `vpn_routes` directly, so the answer is the
//! same whichever pod serves it. Unlike `GetGatewayStatus`, it never shows one
//! pod's in-memory cache. Routes are sorted by IPv4 address; a row whose
//! `local_ip` does not parse is listed last, by its raw text.
//!
//! An RPC error exits non-zero and prints its code, so a database outage is
//! never shown as `0 route(s)`.

use std::net::Ipv4Addr;

use grpc_network_gateway::services::gateway::proto::{
    gateway_controller_client::GatewayControllerClient,
    ListRoutesRequest,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = std::env
        ::var("GATEWAY_ADDR")
        .unwrap_or_else(|_| "http://127.0.0.1:50051".to_string());
    let mut client = GatewayControllerClient::connect(addr).await?;

    let mut routes = client.list_routes(ListRoutesRequest {}).await?.into_inner().routes;

    // Parsed addresses first, in numeric order; unparseable rows after, by raw string.
    routes.sort_by(|a, b| {
        let key = |ip: &str| ip.parse::<Ipv4Addr>().map_or((1, None), |ip| (0, Some(ip)));
        key(&a.local_ip).cmp(&key(&b.local_ip)).then_with(|| a.local_ip.cmp(&b.local_ip))
    });

    // Escaped, so a stored CR/LF or terminal escape cannot break the columns
    // or reach the terminal; widths are measured on the escaped text.
    let lines: Vec<[String; 3]> = routes
        .iter()
        .map(|r| {
            [&r.local_ip, &r.tunnel_id, &r.remote_endpoint].map(|f| f.escape_debug().to_string())
        })
        .collect();
    let width = |i: usize| lines.iter().map(|l| l[i].chars().count()).max().unwrap_or(0);
    let (ip_width, tunnel_width) = (width(0), width(1));
    for [local_ip, tunnel_id, remote_endpoint] in &lines {
        println!("{:<ip_width$}  {:<tunnel_width$}  {}", local_ip, tunnel_id, remote_endpoint);
    }
    println!("{} route(s)", routes.len());
    Ok(())
}
