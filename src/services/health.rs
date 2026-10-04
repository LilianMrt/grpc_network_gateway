//! Readiness reporting wired to real dependencies.
//!
//! `tonic_health` starts the overall service ("") as SERVING unconditionally.
//! That makes a readiness probe worthless: the pod would accept traffic while
//! Postgres is unreachable, and every request would then fail.
//!
//! Readiness here means two things, both of which must hold:
//!   1. the routing table has been hydrated from the database at least once, and
//!   2. the database still answers.
//!
//! Because the pool connects lazily, the process starts and serves the health
//! service even when Postgres is not up yet. It reports NOT_SERVING, and flips
//! to SERVING on its own once the database appears. That is what lets a pod
//! survive being scheduled before its database instead of crash-looping.

use std::time::Duration;

use tokio::task::JoinHandle;
use tonic_health::ServingStatus;
use tonic_health::server::HealthReporter;
use tracing::{ info, warn };

use crate::network::router::RoutingTable;
use crate::services::gateway::hydrate;
use crate::store::Store;

/// The empty service name is the overall-health entry in the gRPC health
/// checking protocol, and is what Kubernetes probes when no service is named.
/// It carries *readiness*: it goes NOT_SERVING whenever the database is
/// unreachable, so the endpoint is pulled out of the Service.
pub const OVERALL: &str = "";

/// A separate entry carrying *liveness*, set SERVING once the gRPC server is
/// listening and never changed afterwards.
///
/// This must not depend on Postgres. Liveness answers "should this container be
/// killed and restarted?", and a database outage is not a reason to restart the
/// process: every replica would fail its liveness probe at once and enter a
/// restart storm that makes the outage worse while fixing nothing. Readiness
/// alone is the correct response to a failed dependency, because it stops
/// traffic without destroying a process that is perfectly capable of recovering.
pub const LIVENESS: &str = "liveness";

/// Hydrates the routing table, then keeps reported health in step with the
/// database. The caller should report NOT_SERVING before serving begins, so no
/// probe can observe SERVING before the first check has actually run.
pub fn spawn_readiness_task(
    reporter: HealthReporter,
    store: Store,
    routing_table: RoutingTable,
    service_name: &'static str,
    interval: Duration
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut hydrated = false;
        let mut last: Option<bool> = None;

        loop {
            let check = if hydrated {
                store.ping().await
            } else {
                hydrate(&store, &routing_table).await.map(|count| {
                    // `count` stays in the message: the FR-10 demo quotes this literal text.
                    info!(count, "hydrated {} route(s) from the database", count);
                    hydrated = true;
                })
            };
            let ready = check.is_ok();

            let status = if ready { ServingStatus::Serving } else { ServingStatus::NotServing };
            reporter.set_service_status(OVERALL, status).await;
            reporter.set_service_status(service_name, status).await;

            // Log transitions only, so a healthy server stays quiet and an
            // outage logs its cause once rather than on every check. Going
            // not-ready is a warning, so RUST_LOG=warn still shows it.
            if last != Some(ready) {
                match &check {
                    Ok(()) => info!(%status, "readiness status changed"),
                    Err(err) => warn!(%status, error = %err, "readiness status changed"),
                }
                last = Some(ready);
            }

            tokio::time::sleep(interval).await;
        }
    })
}
