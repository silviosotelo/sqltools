// SPDX-License-Identifier: GPL-3.0-only
//! Lógica de dominio pura, sin IO.

mod split;

pub use split::{split, statement_at, Statement};
