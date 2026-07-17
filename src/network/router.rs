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
