// SPDX-License-Identifier: GPL-3.0-only
//! SQLTools Rust Oracle driver: Instant Client location, connection,
//! batched queries and cancellation.

pub mod oci;
pub mod session;
pub mod spec;
pub mod wallet;

pub use oci::{locate_oci_dir, prepare_oci_environment, OciError};
pub use oracle::sql_type::ToSql;
pub use session::{QueryHandle, QueryOptions, Session};
pub use spec::{ConnectMode, ConnectSpec, ConnectTarget, SpecError};
