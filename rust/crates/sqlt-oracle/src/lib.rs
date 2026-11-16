// SPDX-License-Identifier: GPL-3.0-only
//! Driver Oracle de SQLTools Rust: localización del Instant Client, conexión,
//! consultas por lotes y cancelación.

pub mod oci;
pub mod session;
pub mod spec;

pub use oci::{locate_oci_dir, prepare_oci_environment, OciError};
pub use session::{QueryHandle, QueryOptions, Session};
pub use spec::{ConnectMode, ConnectSpec, ConnectTarget, SpecError};
