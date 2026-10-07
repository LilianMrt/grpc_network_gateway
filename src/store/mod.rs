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

use std::time::Duration;

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

mod error;

pub use error::{ Error, ErrorKind };

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
    ///
    /// `acquire_timeout` bounds how long a call waits for a connection before
    /// failing `Unavailable`; with Postgres down that is how long every call
    /// takes to fail.
    pub fn connect_lazy(url: &str, acquire_timeout: Duration) -> Result<Self, Error> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(acquire_timeout)
            .connect_lazy(url)?;
        Ok(Self { pool })
    }

    /// A store whose every call fails with `PoolTimedOut`, as production does
    /// when Postgres is down. Port 1 refuses the connection; sqlx retries a
    /// refusal until the acquire deadline, so the timeout is short to keep each
    /// test well under a second instead of sqlx's default 30s.
    /// Built through [`Store::connect_lazy`], so the tests exercise the
    /// production constructor and its timeout parameter.
    #[cfg(test)]
    pub(crate) fn unreachable() -> Self {
        Self::connect_lazy("postgres://netgw@127.0.0.1:1/netgw", Duration::from_millis(200))
            .expect("a well-formed Postgres URL")
    }

    /// Succeeds when the database answers a trivial query.
    pub async fn ping(&self) -> Result<(), Error> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;
        Ok(())
    }

    /// Every persisted route.
    pub async fn list_routes(&self) -> Result<Vec<StoredRoute>, Error> {
        let routes = sqlx::query_as!(
            StoredRoute,
            "SELECT local_ip, tunnel_id, remote_endpoint FROM vpn_routes"
        )
            .fetch_all(&self.pool).await?;
        Ok(routes)
    }

    /// Claims the route for `local_ip` for `owner`: inserts it when absent, or
    /// replaces its tunnel and endpoint when `owner` already holds it. A route
    /// held by another owner is left untouched and reported as
    /// [`ErrorKind::OwnedByAnother`].
    ///
    /// Ownership is decided inside the one statement, never by a read before
    /// the write, so two concurrent creates for one `local_ip` cannot both win.
    pub async fn upsert_route(
        &self,
        local_ip: &str,
        tunnel_id: &str,
        remote_endpoint: &str,
        owner: &str
    ) -> Result<(), Error> {
        let result = sqlx::query!(
            "INSERT INTO vpn_routes (local_ip, tunnel_id, remote_endpoint, owner)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (local_ip)
             DO UPDATE SET tunnel_id = EXCLUDED.tunnel_id, remote_endpoint = EXCLUDED.remote_endpoint
             WHERE vpn_routes.owner = EXCLUDED.owner",
            local_ip,
            tunnel_id,
            remote_endpoint,
            owner
        )
            .execute(&self.pool).await?;
        // The conflict branch's WHERE filters out a foreign row, so nothing is
        // inserted or updated and no row is affected.
        if result.rows_affected() == 0 {
            return Err(Error::owned_by_another(local_ip));
        }
        Ok(())
    }

    /// Removes the route for `local_ip` if `owner` holds it, returning whether
    /// a row was removed. A route held by another owner is left untouched and
    /// reported as `false`, exactly like an absent one.
    pub async fn delete_route(&self, local_ip: &str, owner: &str) -> Result<bool, Error> {
        let result = sqlx
            ::query!("DELETE FROM vpn_routes WHERE local_ip = $1 AND owner = $2", local_ip, owner)
            .execute(&self.pool).await?;
        Ok(result.rows_affected() > 0)
    }
}
