use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct Route {
    pub tunnel_id: String,
    pub remote_endpoint: String,
}

#[derive(Debug, Clone)]
pub struct RoutingTable {
    table: Arc<RwLock<HashMap<Ipv4Addr, Route>>>,
}

impl RoutingTable {
    pub fn new() -> Self {
        Self {
            table: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn add_route(&self, ip: Ipv4Addr, route: Route) {
        let mut write_guard = self.table.write().await;
        write_guard.insert(ip, route);
    }

    pub async fn lookup_route(&self, ip: &Ipv4Addr) -> Option<Route> {
        let read_guard = self.table.read().await;
        read_guard.get(ip).cloned()
    }

    pub async fn get_all_routes(&self) -> HashMap<Ipv4Addr, Route> {
        let read_guard = self.table.read().await;
        read_guard.clone()
    }
}

pub fn parse_destination_ip(packet_bytes: &[u8]) -> Result<Ipv4Addr, String> {
    if packet_bytes.len() < 20 {
        return Err("Packet too short to contain a valid IPv4 header".to_string());
    }

    let version = packet_bytes[0] >> 4;
    if version != 4 {
        return Err(format!("Unsupported IP version: IPv{}", version));
    }

    let dest_bytes: [u8; 4] = [
        packet_bytes[16],
        packet_bytes[17],
        packet_bytes[18],
        packet_bytes[19],
    ];

    Ok(Ipv4Addr::from(dest_bytes))
}
