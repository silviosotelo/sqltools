// SPDX-License-Identifier: GPL-3.0-only
//! Especificación de conexión: usuario, destino, modo y preferencias.

use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectTarget {
    Alias(String),
    /// Cadena de conexión directa (por ejemplo `//host:1521/servicio`).
    Direct(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectMode {
    Normal,
    SysDba,
    SysOper,
}

impl ConnectMode {
    pub fn parse(s: &str) -> Option<ConnectMode> {
        match s.trim().to_ascii_uppercase().as_str() {
            "" | "NORMAL" => Some(ConnectMode::Normal),
            "SYSDBA" => Some(ConnectMode::SysDba),
            "SYSOPER" => Some(ConnectMode::SysOper),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectSpec {
    pub user: String,
    pub password: String,
    pub target: ConnectTarget,
    pub mode: ConnectMode,
    pub read_only: bool,
    pub oci_dir: Option<PathBuf>,
    pub tns_admin: Option<PathBuf>,
    pub prefetch: u32,
}

#[derive(Debug, PartialEq, Eq)]
pub enum SpecError {
    Missing(&'static str),
    InvalidMode(String),
}

impl fmt::Display for SpecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpecError::Missing(name) => write!(f, "Falta la variable de entorno {name}"),
            SpecError::InvalidMode(value) => {
                write!(
                    f,
                    "Modo de conexión inválido: {value} (usá NORMAL, SYSDBA o SYSOPER)"
                )
            }
        }
    }
}

impl std::error::Error for SpecError {}

fn parse_bool(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "si" | "sí"
    )
}

impl ConnectSpec {
    pub fn from_env() -> Result<Self, SpecError> {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    /// `lookup` permite probar el parseo sin tocar el entorno del proceso.
    pub fn from_lookup<F>(lookup: F) -> Result<Self, SpecError>
    where
        F: Fn(&str) -> Option<String>,
    {
        let user = lookup("SQLT_USER").ok_or(SpecError::Missing("SQLT_USER"))?;
        let password = lookup("SQLT_PASS").ok_or(SpecError::Missing("SQLT_PASS"))?;
        let target = match (lookup("SQLT_CONNECT"), lookup("SQLT_ALIAS")) {
            (Some(connect), _) if !connect.trim().is_empty() => {
                ConnectTarget::Direct(connect.trim().to_string())
            }
            (_, Some(alias)) if !alias.trim().is_empty() => {
                ConnectTarget::Alias(alias.trim().to_string())
            }
            _ => return Err(SpecError::Missing("SQLT_ALIAS o SQLT_CONNECT")),
        };
        let mode_raw = lookup("SQLT_MODE").unwrap_or_default();
        let mode = ConnectMode::parse(&mode_raw).ok_or(SpecError::InvalidMode(mode_raw))?;
        let read_only = lookup("SQLT_READ_ONLY")
            .map(|v| parse_bool(&v))
            .unwrap_or(false);
        let prefetch = lookup("SQLT_PREFETCH")
            .and_then(|v| v.trim().parse::<u32>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(1000);
        Ok(ConnectSpec {
            user,
            password,
            target,
            mode,
            read_only,
            oci_dir: lookup("SQLT_OCI_DIR").map(PathBuf::from),
            tns_admin: lookup("SQLT_TNS_ADMIN").map(PathBuf::from),
            prefetch,
        })
    }

    pub fn connect_string(&self) -> String {
        match &self.target {
            ConnectTarget::Alias(a) => a.clone(),
            ConnectTarget::Direct(s) => s.clone(),
        }
    }

    pub fn direct(user: &str, password: &str, host: &str, port: u16, service: &str) -> Self {
        ConnectSpec {
            user: user.to_string(),
            password: password.to_string(),
            target: ConnectTarget::Direct(format!("//{host}:{port}/{service}")),
            mode: ConnectMode::Normal,
            read_only: false,
            oci_dir: None,
            tns_admin: None,
            prefetch: 1000,
        }
    }
}
