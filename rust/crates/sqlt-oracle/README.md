# sqlt-oracle

Driver Oracle de SQLTools Rust, sobre el crate [`oracle`](https://crates.io/crates/oracle)
(ODPI-C). El Instant Client se carga en runtime; **no** se incluye en el repo.

## Variables de entorno

| Variable | Uso |
|---|---|
| `SQLT_USER`, `SQLT_PASS` | Credenciales (obligatorias) |
| `SQLT_ALIAS` | Alias de `tnsnames.ora` |
| `SQLT_CONNECT` | Cadena directa, p. ej. `//host:1521/servicio` (tiene prioridad sobre el alias) |
| `SQLT_TNS_ADMIN` | Carpeta con `tnsnames.ora`/wallet |
| `SQLT_OCI_DIR` | Carpeta del Instant Client (si no, `DPI_OCI_LIB_DIR`, `PATH`, candidatos) |
| `SQLT_MODE` | `NORMAL` (defecto), `SYSDBA`, `SYSOPER` |
| `SQLT_READ_ONLY` | `1`/`true` bloquea todo lo que no sea SELECT/WITH/EXPLAIN/DESCRIBE |
| `SQLT_PREFETCH` | Filas por viaje de red (defecto 1000) |

## Pruebas de integración

Se saltan si faltan credenciales/destino. Ejemplo (PowerShell):

```powershell
$env:SQLT_USER="usuario"; $env:SQLT_PASS="clave"; $env:SQLT_ALIAS="santa"
$env:SQLT_TNS_ADMIN="C:\instantclient_23_5\network\admin"
$env:SQLT_OCI_DIR="C:\instantclient_23_5"
cargo test -p sqlt-oracle --test oracle_integration -- --nocapture
```

Cubre `select 1 from dual`, tipos comunes, cancelación con `ORA-01013` y
sesión reutilizable, lotes de 100 filas y modo de solo lectura.

## CLI

```powershell
cargo run -p sqlt-cli -- diag
cargo run -p sqlt-cli -- query "select 1 from dual"
cargo run -p sqlt-cli -- bench "select * from all_objects where rownum <= 10000" --runs 3
```
