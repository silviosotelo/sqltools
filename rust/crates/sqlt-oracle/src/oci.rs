// SPDX-License-Identifier: GPL-3.0-only
//! Localización del Oracle Instant Client y arranque de ODPI-C.
//!
//! Prioridad: `DPI_OCI_LIB_DIR` > `SQLT_OCI_DIR` > `PATH` > candidatos del
//! sistema. Nunca se redistribuye el cliente; solo se lo ubica.

use std::ffi::OsStr;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[derive(Debug)]
pub enum OciError {
    NotFound { searched: Vec<String> },
    Init(String),
    ReadOnly,
    Unsupported(String),
    Oracle(oracle::Error),
}

impl fmt::Display for OciError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OciError::NotFound { searched } => write!(
                f,
                "No se encontró el Oracle Instant Client (oci.dll / libclntsh). \
                 Definí SQLT_OCI_DIR o DPI_OCI_LIB_DIR. Buscado en: {}",
                searched.join(", ")
            ),
            OciError::Init(msg) => write!(f, "No se pudo inicializar ODPI-C: {msg}"),
            OciError::ReadOnly => write!(
                f,
                "La conexión es de solo lectura: solo se permiten SELECT/WITH/EXPLAIN/DESCRIBE"
            ),
            OciError::Unsupported(msg) => write!(f, "{msg}"),
            OciError::Oracle(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for OciError {}

impl From<oracle::Error> for OciError {
    fn from(e: oracle::Error) -> Self {
        OciError::Oracle(e)
    }
}

fn has_oci_library(dir: &Path) -> bool {
    if !dir.is_dir() {
        return false;
    }
    if dir.join("oci.dll").is_file() {
        return true;
    }
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .any(|e| e.file_name().to_string_lossy().starts_with("libclntsh"))
        })
        .unwrap_or(false)
}

/// Elige el primer directorio que contenga el cliente.
pub fn locate_oci_dir_in(
    dpi_oci_lib_dir: Option<&Path>,
    sqlt_oci_dir: Option<&Path>,
    path_env: Option<&OsStr>,
    candidates: &[PathBuf],
) -> Option<PathBuf> {
    let direct = dpi_oci_lib_dir.into_iter().chain(sqlt_oci_dir);
    for dir in direct {
        if has_oci_library(dir) {
            return Some(dir.to_path_buf());
        }
    }
    let mut searched: Vec<PathBuf> = Vec::new();
    if let Some(path_env) = path_env {
        for dir in std::env::split_paths(path_env) {
            searched.push(dir.clone());
            if has_oci_library(&dir) {
                return Some(dir);
            }
        }
    }
    for dir in candidates {
        if has_oci_library(dir) {
            return Some(dir.clone());
        }
    }
    None
}

fn discover_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    #[cfg(windows)]
    {
        for root in [PathBuf::from("C:\\"), PathBuf::from("C:\\oracle")] {
            if let Ok(entries) = std::fs::read_dir(&root) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir()
                        && entry
                            .file_name()
                            .to_string_lossy()
                            .to_ascii_lowercase()
                            .starts_with("instantclient")
                    {
                        out.push(path);
                    }
                }
            }
        }
    }
    #[cfg(not(windows))]
    {
        let mut roots: Vec<PathBuf> = vec![
            PathBuf::from("/opt/oracle"),
            PathBuf::from("/usr/local/lib"),
            PathBuf::from("/usr/lib/oracle"),
        ];
        if let Some(home) = std::env::var_os("HOME") {
            roots.push(PathBuf::from(home));
        }
        for root in roots {
            if let Ok(entries) = std::fs::read_dir(&root) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir()
                        && entry
                            .file_name()
                            .to_string_lossy()
                            .to_ascii_lowercase()
                            .starts_with("instantclient")
                    {
                        out.push(path);
                    }
                }
            }
        }
        if let Ok(entries) = std::fs::read_dir("/usr/lib/oracle") {
            for entry in entries.flatten() {
                let lib = entry.path().join("client64").join("lib");
                if lib.is_dir() {
                    out.push(lib);
                }
            }
        }
    }
    out
}

/// Busca el cliente con el entorno real del proceso.
pub fn locate_oci_dir() -> Result<PathBuf, OciError> {
    let dpi = std::env::var_os("DPI_OCI_LIB_DIR").map(PathBuf::from);
    let sqlt = std::env::var_os("SQLT_OCI_DIR").map(PathBuf::from);
    let path = std::env::var_os("PATH");
    let candidates = discover_candidates();
    if let Some(dir) = locate_oci_dir_in(
        dpi.as_deref(),
        sqlt.as_deref(),
        path.as_deref(),
        &candidates,
    ) {
        return Ok(dir);
    }
    let mut searched = Vec::new();
    if let Some(d) = dpi {
        searched.push(d.display().to_string());
    }
    if let Some(d) = sqlt {
        searched.push(d.display().to_string());
    }
    if let Some(p) = std::env::var_os("PATH") {
        searched.extend(
            std::env::split_paths(&p)
                .take(8)
                .map(|d| d.display().to_string()),
        );
    }
    searched.extend(candidates.iter().map(|p| p.display().to_string()));
    Err(OciError::NotFound { searched })
}

static INIT: OnceLock<Result<(), String>> = OnceLock::new();

/// Inicializa ODPI-C una sola vez. La primera llamada fija el cliente y el
/// `TNS_ADMIN`; las siguientes reutilizan esa configuración.
pub fn prepare_oci_environment(lib_dir: &Path, config_dir: Option<&Path>) -> Result<(), OciError> {
    let result = INIT.get_or_init(|| {
        let mut params = oracle::InitParams::new();
        if let Err(e) = params.oracle_client_lib_dir(lib_dir) {
            return Err(e.to_string());
        }
        if let Some(dir) = config_dir {
            if let Err(e) = params.oracle_client_config_dir(dir) {
                return Err(e.to_string());
            }
        }
        params.init().map(|_| ()).map_err(|e| e.to_string())
    });
    result.clone().map_err(OciError::Init)
}
