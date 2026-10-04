//! The only path to Postgres.
//!
//! Every SQL statement, and the pool that runs them, lives in this module.
//! Nothing outside it names sqlx: callers hold a [`Store`] and get plain Rust
//! values back.
//!
//! The dependency is one-way. Callers that keep the in-memory routing table in
//! step with the database call the store first and update the cache second.
//! The store never imports the network module or touches the cache, and the
//! routing table knows nothing about the store.

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

/// The error every store call returns. A plain alias for now: it is the seam a
/// later failure taxonomy replaces without touching the callers' signatures.
pub type Error = sqlx::Error;

/// One persisted row of `vpn_routes`, as stored. `local_ip` is not parsed
/// here: deciding what to do with a malformed row is the caller's business.
#[derive(Clone, Debug)]
pub struct StoredRoute {
    pub local_ip: String,
    pub tunnel_id: String,
    pub remote_endpoint: String,
}

/// A cheap, cloneable handle on the connection pool.
#[derive(Clone, Debug)]
pub struct Store {
    pool: PgPool,
}

impl Store {
    /// Builds the pool without connecting. Constructing it must not require
    /// Postgres to be up, so a process started before its database reports
    /// not-ready instead of failing to start.
    pub fn connect_lazy(url: &str) -> Result<Self, Error> {
        let pool = PgPoolOptions::new().max_connections(5).connect_lazy(url)?;
        Ok(Self { pool })
    }

    /// Succeeds when the database answers a trivial query.
    pub async fn ping(&self) -> Result<(), Error> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;
        Ok(())
    }

    /// Every persisted route.
    pub async fn list_routes(&self) -> Result<Vec<StoredRoute>, Error> {
        sqlx::query_as!(
            StoredRoute,
            "SELECT local_ip, tunnel_id, remote_endpoint FROM vpn_routes"
        )
            .fetch_all(&self.pool).await
    }

    /// Inserts the route for `local_ip`, or replaces its tunnel and endpoint
    /// when one already exists.
    pub async fn upsert_route(
        &self,
        local_ip: &str,
        tunnel_id: &str,
        remote_endpoint: &str
    ) -> Result<(), Error> {
        sqlx::query!(
            "INSERT INTO vpn_routes (local_ip, tunnel_id, remote_endpoint)
             VALUES ($1, $2, $3)
             ON CONFLICT (local_ip)
             DO UPDATE SET tunnel_id = EXCLUDED.tunnel_id, remote_endpoint = EXCLUDED.remote_endpoint",
            local_ip,
            tunnel_id,
            remote_endpoint
        )
            .execute(&self.pool).await?;
        Ok(())
    }

    /// Removes the route for `local_ip`, returning whether a row existed.
    pub async fn delete_route(&self, local_ip: &str) -> Result<bool, Error> {
        let result = sqlx
            ::query!("DELETE FROM vpn_routes WHERE local_ip = $1", local_ip)
            .execute(&self.pool).await?;
        Ok(result.rows_affected() > 0)
    }
}
