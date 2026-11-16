// SPDX-License-Identifier: GPL-3.0-only
//! Lógica de dominio pura, sin IO.

pub mod binds;
pub mod datefmt;
pub mod readonly;
mod split;
pub mod value;

pub use split::{split, statement_at, Statement};
pub use value::{Batch, ColumnMeta, Row, Value};
