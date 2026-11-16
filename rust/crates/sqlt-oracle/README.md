# sqlt-oracle

SQLTools Rust Oracle driver, built on the [`oracle`](https://crates.io/crates/oracle)
crate (ODPI-C). The Instant Client is loaded at runtime and **not** shipped in
the repository.

## Environment variables

| Variable | Purpose |
|---|---|
| `SQLT_USER`, `SQLT_PASS` | Credentials (required) |
| `SQLT_ALIAS` | `tnsnames.ora` alias |
| `SQLT_CONNECT` | Direct string, e.g. `//host:1521/service` (takes precedence over the alias) |
| `SQLT_TNS_ADMIN` | Folder containing `tnsnames.ora`/wallet |
| `SQLT_OCI_DIR` | Instant Client folder (otherwise `DPI_OCI_LIB_DIR`, `PATH`, candidates) |
| `SQLT_MODE` | `NORMAL` (default), `SYSDBA`, `SYSOPER` |
| `SQLT_READ_ONLY` | `1`/`true` blocks everything except SELECT/WITH/EXPLAIN/DESCRIBE |
| `SQLT_PREFETCH` | Rows per network round trip (default 1000) |

## Integration tests

They are skipped when credentials/target are missing. Example (PowerShell):

```powershell
$env:SQLT_USER="user"; $env:SQLT_PASS="password"; $env:SQLT_ALIAS="santa"
$env:SQLT_TNS_ADMIN="C:\instantclient_23_5\network\admin"
$env:SQLT_OCI_DIR="C:\instantclient_23_5"
cargo test -p sqlt-oracle --test oracle_integration -- --nocapture
```

Covers `select 1 from dual`, common types, cancellation with `ORA-01013` and a
reusable session, 100k-row batches and read-only mode. The ignored
`golden_dates_260k` compares 260,052 dates × 13 formats against `TO_CHAR`.

## CLI

```powershell
cargo run -p sqlt-cli -- diag
cargo run -p sqlt-cli -- query "select 1 from dual"
cargo run -p sqlt-cli -- bench "select * from all_objects where rownum <= 10000" --runs 3
```
