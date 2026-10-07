//! The one failure taxonomy every store call returns (FR-33, AD-3).
//!
//! A database failure is classified exactly once, here, when it leaves the
//! store, and turned into a gRPC [`Status`] by exactly one `From` impl. Any RPC
//! that calls the store inherits the same codes without mapping anything
//! itself:
//!
//! | Kind              | gRPC code             | Meaning for a caller           |
//! |-------------------|-----------------------|--------------------------------|
//! | `Unavailable`     | `UNAVAILABLE`         | retry: the database is away    |
//! | `InvalidArgument` | `INVALID_ARGUMENT`    | permanent: fix the request     |
//! | `OwnedByAnother`  | `FAILED_PRECONDITION` | another resource holds the row |
//! | `Internal`        | `INTERNAL`            | unexpected; a bug or bad schema|
//!
//! Classification reads the sqlx error variant and the SQLSTATE code, never
//! the message text, which differs between server versions and locales.

use std::fmt;

use tonic::Status;

/// Which of the four outcomes a store failure is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    /// The database cannot be reached, or refuses connections for now. Retryable.
    Unavailable,
    /// The database rejected the values themselves (SQLSTATE classes `22`
    /// and `23`). Permanent: retrying the same request fails the same way.
    InvalidArgument,
    /// The row exists and another owner holds it; nothing was written.
    OwnedByAnother,
    /// Anything else, including a stale schema (class `42`).
    Internal,
}

/// The error every [`Store`](super::Store) call returns.
#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
    repr: Repr,
}

#[derive(Debug)]
enum Repr {
    Database(sqlx::Error),
    OwnedByAnother { local_ip: String },
}

impl Error {
    /// The route for `local_ip` belongs to another owner.
    pub(crate) fn owned_by_another(local_ip: &str) -> Self {
        Self {
            kind: ErrorKind::OwnedByAnother,
            repr: Repr::OwnedByAnother { local_ip: local_ip.to_string() },
        }
    }

    pub fn kind(&self) -> ErrorKind {
        self.kind
    }
}

impl From<sqlx::Error> for Error {
    fn from(err: sqlx::Error) -> Self {
        Self { kind: classify(&err), repr: Repr::Database(err) }
    }
}

/// The classifier. Connection-level failures are `Unavailable` whatever the
/// statement was; a server-side error is decided by its SQLSTATE alone.
fn classify(err: &sqlx::Error) -> ErrorKind {
    match err {
        | sqlx::Error::PoolTimedOut
        | sqlx::Error::PoolClosed
        | sqlx::Error::Io(_)
        | sqlx::Error::Tls(_)
        | sqlx::Error::WorkerCrashed => ErrorKind::Unavailable,
        sqlx::Error::Database(db) =>
            match db.code() {
                Some(code) => classify_sqlstate(&code),
                None => ErrorKind::Internal,
            }
        _ => ErrorKind::Internal,
    }
}

fn classify_sqlstate(code: &str) -> ErrorKind {
    match code {
        // 08 connection exception, 53 insufficient resources (too many
        // connections, disk full), 57P01-57P03 admin/crash shutdown and
        // "cannot connect now" while the server starts.
        c if c.starts_with("08") || c.starts_with("53") => ErrorKind::Unavailable,
        "57P01" | "57P02" | "57P03" => ErrorKind::Unavailable,
        // 22 data exception (e.g. 22001 value too long), 23 integrity
        // constraint violation (e.g. 23502 not null).
        c if c.starts_with("22") || c.starts_with("23") => ErrorKind::InvalidArgument,
        _ => ErrorKind::Internal,
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.repr {
            // The message never names the holder: it belongs to another namespace.
            Repr::OwnedByAnother { local_ip } =>
                write!(f, "Tunnel for {} is owned by another resource", local_ip),
            Repr::Database(source) =>
                match self.kind {
                    ErrorKind::Unavailable => write!(f, "database unavailable: {}", source),
                    ErrorKind::InvalidArgument => write!(f, "rejected by the database: {}", source),
                    ErrorKind::OwnedByAnother | ErrorKind::Internal =>
                        write!(f, "unexpected database failure: {}", source),
                }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.repr {
            Repr::Database(source) => Some(source),
            Repr::OwnedByAnother { .. } => None,
        }
    }
}

/// The only place a store failure becomes a gRPC status.
impl From<Error> for Status {
    fn from(err: Error) -> Self {
        let message = err.to_string();
        match err.kind {
            ErrorKind::Unavailable => Status::unavailable(message),
            ErrorKind::InvalidArgument => Status::invalid_argument(message),
            ErrorKind::OwnedByAnother => Status::failed_precondition(message),
            ErrorKind::Internal => Status::internal(message),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::borrow::Cow;
    use tonic::Code;

    /// A server-side error carrying only a SQLSTATE, as the Postgres driver
    /// reports one.
    #[derive(Debug)]
    struct FakeDbError(Option<&'static str>);

    impl fmt::Display for FakeDbError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "fake database error {:?}", self.0)
        }
    }

    impl std::error::Error for FakeDbError {}

    impl sqlx::error::DatabaseError for FakeDbError {
        fn message(&self) -> &str {
            "fake database error"
        }

        fn code(&self) -> Option<Cow<'_, str>> {
            self.0.map(Cow::Borrowed)
        }

        fn as_error(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
            self
        }

        fn as_error_mut(&mut self) -> &mut (dyn std::error::Error + Send + Sync + 'static) {
            self
        }

        fn into_error(self: Box<Self>) -> Box<dyn std::error::Error + Send + Sync + 'static> {
            self
        }

        fn kind(&self) -> sqlx::error::ErrorKind {
            sqlx::error::ErrorKind::Other
        }
    }

    fn sqlstate(code: &'static str) -> sqlx::Error {
        sqlx::Error::Database(Box::new(FakeDbError(Some(code))))
    }

    #[test]
    fn classifies_by_variant_and_sqlstate() {
        let cases: Vec<(&str, sqlx::Error, ErrorKind, Code)> = vec![
            ("22001", sqlstate("22001"), ErrorKind::InvalidArgument, Code::InvalidArgument),
            ("23502", sqlstate("23502"), ErrorKind::InvalidArgument, Code::InvalidArgument),
            ("08006", sqlstate("08006"), ErrorKind::Unavailable, Code::Unavailable),
            ("53300", sqlstate("53300"), ErrorKind::Unavailable, Code::Unavailable),
            ("57P01", sqlstate("57P01"), ErrorKind::Unavailable, Code::Unavailable),
            ("57P02", sqlstate("57P02"), ErrorKind::Unavailable, Code::Unavailable),
            ("57P03", sqlstate("57P03"), ErrorKind::Unavailable, Code::Unavailable),
            ("57P04", sqlstate("57P04"), ErrorKind::Internal, Code::Internal),
            (
                "no SQLSTATE",
                sqlx::Error::Database(Box::new(FakeDbError(None))),
                ErrorKind::Internal,
                Code::Internal,
            ),
            ("57014", sqlstate("57014"), ErrorKind::Internal, Code::Internal),
            ("42703", sqlstate("42703"), ErrorKind::Internal, Code::Internal),
            ("PoolTimedOut", sqlx::Error::PoolTimedOut, ErrorKind::Unavailable, Code::Unavailable),
            ("PoolClosed", sqlx::Error::PoolClosed, ErrorKind::Unavailable, Code::Unavailable),
            ("Tls", sqlx::Error::Tls("handshake failed".into()), ErrorKind::Unavailable, Code::Unavailable),
            ("WorkerCrashed", sqlx::Error::WorkerCrashed, ErrorKind::Unavailable, Code::Unavailable),
            (
                "Io",
                sqlx::Error::Io(std::io::Error::from(std::io::ErrorKind::ConnectionRefused)),
                ErrorKind::Unavailable,
                Code::Unavailable,
            ),
            ("Protocol", sqlx::Error::Protocol("garbled".into()), ErrorKind::Internal, Code::Internal),
            ("Decode", sqlx::Error::Decode("bad column".into()), ErrorKind::Internal, Code::Internal)
        ];

        for (name, source, kind, code) in cases {
            let err = Error::from(source);
            assert_eq!(err.kind(), kind, "{name}");
            assert_eq!(Status::from(err).code(), code, "{name}");
        }
    }

    #[test]
    fn status_messages_name_the_class() {
        let unavailable = Status::from(Error::from(sqlx::Error::PoolTimedOut));
        assert!(unavailable.message().starts_with("database unavailable: "), "{unavailable:?}");

        let rejected = Status::from(Error::from(sqlstate("22001")));
        assert!(rejected.message().starts_with("rejected by the database: "), "{rejected:?}");

        let internal = Status::from(Error::from(sqlstate("42703")));
        assert!(internal.message().starts_with("unexpected database failure: "), "{internal:?}");
    }

    #[test]
    fn owned_by_another_is_failed_precondition_and_never_names_the_holder() {
        let err = Error::owned_by_another("10.0.0.5");
        assert_eq!(err.kind(), ErrorKind::OwnedByAnother);
        let status = Status::from(err);
        assert_eq!(status.code(), Code::FailedPrecondition);
        assert_eq!(status.message(), "Tunnel for 10.0.0.5 is owned by another resource");
    }
}
