# SQLTools Rust — P1a: motor Oracle (vertical slice)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans. Steps are `- [ ]` checkboxes.

**Goal:** Un CLI Rust que conecte a Oracle con el Instant Client, ejecute SQL, transmita resultados tipados por lotes y cancele, con las piezas puras (fechas y binds) cubiertas por pruebas de paridad.

**Architecture:** `sqlt-core` (puro) + `sqlt-oracle` (driver sobre el crate `oracle`/ODPI-C) + `sqlt-cli` (harness). La app Tauri consumirá estas mismas piezas en P2.

**Tech Stack:** Rust 1.100, crate `oracle` 0.6.3, Instant Client 23.5 en `C:\instantclient_23_5`, tokio solo cuando haga falta (el crate es síncrono; el wrapper async llega en P2).

**Spec:** `docs/superpowers/specs/2026-11-16-rust-migration-design.md`

## Global Constraints

- Offsets de texto en UTF-16 (editor); valores NUMBER viajan como string canónica (sin pérdida de 38 dígitos).
- Contraseñas solo por variables de entorno o keyring; nunca en el repo ni en logs.
- Instant Client no se redistribuye; se localiza en runtime.
- Pruebas de integración se saltan si faltan `SQLT_USER`/`SQLT_PASS` y el destino (`SQLT_ALIAS` o `SQLT_CONNECT`).
- GPLv3 en cada crate (`license = "GPL-3.0-only"`).

## Review Focus

1. Precisión de NUMBER: nunca convertir a `f64` si el valor puede superar 15 dígitos.
2. Fechas: `DATE`/`TIMESTAMP` formateadas en cliente y comparadas contra `TO_CHAR`; NLS no inglés usa el camino del driver, no el formateador propio.
3. Cancelación: `break_execution` deja la sesión usable (probar con una consulta larga y luego `select 1 from dual`).
4. LOBs/NULL: un NULL no debe convertirse en la cadena "NULL"; LOB vacío no debe romper el lote.
5. Localización del Instant Client: mensaje claro si falta, sin panic; `DPI_OCI_LIB_DIR` y `SQLT_OCI_DIR` tienen prioridad.

---

### Task 1: Modelo de valores y formateador de fechas (`sqlt-core`)

**Files:**
- Create: `rust/crates/sqlt-core/src/value.rs`
- Create: `rust/crates/sqlt-core/src/datefmt.rs`
- Modify: `rust/crates/sqlt-core/src/lib.rs`
- Test: `rust/crates/sqlt-core/tests/datefmt_test.rs`
- Reference: `src/Common/FastDateFormat.cpp` (port), `etapa1/sqlt_datefmt_test.cpp` (formatos)

**Interfaces:**
```rust
pub enum Value { Null, Text(String), Number(String), Date(String), Timestamp(String), Binary(Vec<u8>) }
pub struct ColumnMeta { pub name: String, pub type_name: String, pub nullable: bool, pub numeric: bool }
pub fn compile_date_format(fmt: &str) -> Option<DateFormat>;
impl DateFormat { pub fn format(&self, y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> String; }
pub fn client_language_is_english() -> bool;
```

- [ ] **Step 1: Pruebas que fallan**

`datefmt_test.rs`: un caso por formato soportado del test C++ (`DD-MON-RR`, `DD-MON-YYYY`, `YYYY-MM-DD HH24:MI:SS`, `YYYY-MM-DD`, `DD/MM/YYYY`, `DD/MM/YYYY HH24:MI:SS`, `DD-MON-RR HH.MI.SS AM`, `MM/DD/YYYY HH:MI:SS AM`, `Day, Month DD, YYYY`, `DY DD Mon YYYY`, `DAY DD-MON-YY`, `YYYY-MM-DD"T"HH24:MI:SS`, `DDD YYYY Q`) con fechas fijas: `2024-02-29 13:45:09`, `1900-03-01 00:00:00`, `2026-01-01 23:59:59`. Y que `dd-mon-rr`, `FMDD-MON-YY`, `DD-MON-RR HH24:MI:SSXFF` devuelvan `None`.
- [ ] **Step 2: Ver fallar** (`cargo test -p sqlt-core datefmt`)
- [ ] **Step 3: Portar `FastDateFormat.cpp`** a `datefmt.rs` (elementos Y4/Y3/Y2/MM/MON/MONTH/DD/DDD/HH12/HH24/MI/SS/AM-PM/DY/DAY/Q, literales entre comillas, nombres en inglés; `client_language_is_english` lee `NLS_LANG`/`NLS_DATE_LANGUAGE`).
- [ ] **Step 4: Implementar `value.rs`** con `Value`, `ColumnMeta`, `impl Value { pub fn is_null(&self) -> bool; pub fn display(&self) -> Option<&str> }` (NULL → `None`).
- [ ] **Step 5: Verde + commit**

Run: `cargo test -p sqlt-core` → PASS. Commit: `feat(core): modelo de valores y formateador rápido de fechas`.

---

### Task 2: Binds (`sqlt-core`)

**Files:**
- Create: `rust/crates/sqlt-core/src/binds.rs`
- Test: `rust/crates/sqlt-core/tests/binds_test.rs`
- Reference: `ui/BindParser.cpp`, `tests/bind_test.cpp`

**Interfaces:**
```rust
pub enum BindKind { Auto, String, Number, Date, Null }
pub struct BindValue { pub kind: BindKind, pub value: String }
pub fn names(sql: &str) -> Vec<String>;
pub fn substitute(sql: &str, values: &HashMap<String, BindValue>) -> (String, Vec<String>);
```

- [ ] **Step 1: Portar los 12 casos de `tests/bind_test.cpp`** a `binds_test.rs` (nombres únicos, ignorar textos/comentarios/q-quotes, `:=`/`::`, posicionales, comillas; sustitución Auto/String/Number/Date/NULL, `''` escapada, `to_date`, avisos por inválidos, sin distinguir mayúsculas, variable ausente = NULL).
- [ ] **Step 2: Ver fallar**
- [ ] **Step 3: Implementar** reutilizando el scanner de `split.rs` para saltar textos/comentarios (extraer un helper si hace falta).
- [ ] **Step 4: Verde + commit** (`feat(core): variables de enlace y sustitución por literales`).

---

### Task 3: Localización del Instant Client y configuración de conexión (`sqlt-oracle`)

**Files:**
- Create: `rust/crates/sqlt-oracle/Cargo.toml` (`oracle = "0.6.3"`, `thiserror`, `sqlt-core = { path = "../sqlt-core" }`, `license = "GPL-3.0-only"`)
- Create: `rust/crates/sqlt-oracle/src/lib.rs`, `src/oci.rs`, `src/spec.rs`
- Test: `rust/crates/sqlt-oracle/tests/spec_test.rs`

**Interfaces:**
```rust
pub fn locate_oci_dir() -> Result<PathBuf, OciError>;                 // DPI_OCI_LIB_DIR > SQLT_OCI_DIR > PATH > candidatos
pub fn prepare_oci_environment(dir: &Path);                            // set DPI_OCI_LIB_DIR + Path (una vez)
pub enum ConnectTarget { Alias(String), Direct { host: String, port: u16, service: String } }
pub enum ConnectMode { Normal, SysDba, SysOper }
pub struct ConnectSpec { pub user: String, pub password: String, pub target: ConnectTarget, pub mode: ConnectMode, pub read_only: bool }
impl ConnectSpec { pub fn from_env() -> Result<Self, SpecError>; pub fn connect_string(&self) -> String; }
```

- [ ] **Step 1: Pruebas que fallan** para prioridad de variables, candidatos con `oci.dll`/`libclntsh.*` en un temp dir, error cuando no hay cliente, `connect_string()` directo (`//host:port/service`) y alias, y parseo de `SQLT_MODE` (SYSDBA/SYSOPER).
- [ ] **Step 2: Implementar** con `locate_oci_dir_in(...)` puro y testeable; `prepare_oci_environment` idempotente (`OnceLock`).
- [ ] **Step 3: Verde + clippy + commit** (`feat(oracle): localización del Instant Client y especificación de conexión`).

---

### Task 4: Sesión, consulta por lotes y cancelación (`sqlt-oracle`)

**Files:**
- Create: `rust/crates/sqlt-oracle/src/session.rs`, `src/types.rs`
- Test: `rust/crates/sqlt-oracle/tests/oracle_integration.rs` (gated)

**Interfaces:**
```rust
pub struct Session { /* oracle::Connection */ }
impl Session {
    pub fn open(spec: &ConnectSpec) -> Result<Session, OciError>;
    pub fn query(&self, sql: &str) -> Result<QueryHandle, OciError>;
    pub fn execute(&self, sql: &str) -> Result<u64, OciError>;   // filas afectadas
    pub fn commit(&self) -> Result<(), OciError>;
    pub fn rollback(&self) -> Result<(), OciError>;
    pub fn cancel(&self) -> Result<(), OciError>;               // break_execution
    pub fn server_version(&self) -> Result<String, OciError>;
}
pub struct QueryHandle { /* ResultSet<'static, Row> */ }
impl QueryHandle { pub fn columns(&self) -> &[ColumnMeta]; pub fn next_batch(&mut self, max: usize) -> Result<Batch, OciError>; }
```

- [ ] **Step 1: Integración que falla (si hay entorno)** — `connect_y_select_dual`, `tipos_comunes` (`1`, `1.5`, `'ñ😀'`, `date '2024-02-29'`, `timestamp '2024-02-29 13:45:09.123'`, `null`), `cancelar_consulta_larga` (cancelar a los 500 ms; error ORA-01013; luego `select 1 from dual` OK), `recorrer_100k_filas_por_lotes` (que el lote respete `max`).
- [ ] **Step 2: Implementar el mapeo** usando `row.sql_values()` + `row.column_info()`: texto → `String`; NUMBER → `String`; DATE/TIMESTAMP → `oracle::sql_type::Timestamp` + `datefmt`; RAW → `Vec<u8>`; NULL → `Value::Null`. LOBs básicos vía `get::<String>`/`Vec<u8>`; si el crate no lo permite, devolver error tipado y documentarlo (P1b).
- [ ] **Step 3: Verde local con credenciales** (ver comando abajo) y commit.

Environment para integración:
```powershell
$env:SQLT_USER="..."; $env:SQLT_PASS="..."; $env:SQLT_ALIAS="santa"
$env:SQLT_TNS_ADMIN="C:\instantclient_23_5\network\admin"
$env:SQLT_OCI_DIR="C:\instantclient_23_5"
cargo test -p sqlt-oracle --test oracle_integration -- --nocapture
```

---

### Task 5: CLI `sqlt-cli`

**Files:**
- Create: `rust/crates/sqlt-cli/Cargo.toml`, `src/main.rs`
- Test: unit tests de parseo en `src/main.rs` o `tests/cli_args_test.rs`

**Interfaces:**
```
sqlt-cli diag
sqlt-cli query "<sql>" [--max-rows N] [--tsv]
sqlt-cli bench "<sql>" [--runs N]
```

- [ ] **Step 1: Pruebas que fallan** del parseo de argumentos (comandos, `--max-rows`, `--runs`, SQL faltante → error de uso, `--help`).
- [ ] **Step 2: Implementar** sin dependencias nuevas (parseo manual); `diag` imprime OCI dir, TNS_ADMIN, versión del servidor o el error; `query` imprime TSV con cabeceras; `bench` mide filas/s.
- [ ] **Step 3: Probar contra Oracle real** (`query "select 1, 'ñ', date '2024-02-29' from dual"`, `bench "select * from all_objects where rownum <= 10000"`).
- [ ] **Step 4: Commit** (`feat(cli): harness de consulta, diagnóstico y benchmark`).

---

### Task 6: Modo solo lectura y CI

**Files:**
- Create: `rust/crates/sqlt-core/src/readonly.rs`
- Test: `rust/crates/sqlt-core/tests/readonly_test.rs`
- Modify: `.github/workflows/rust-ci.yml` (test workspace completo)
- Create: `rust/crates/sqlt-oracle/README.md` (variables y prueba de integración)

- [ ] **Step 1: Prueba que falla**: `is_read_only("select ...")` true; `with ... select` true; `insert/update/delete/create/drop/begin` false; comentarios y blancos antes del verbo se ignoran.
- [ ] **Step 2: Implementar** usando `split`/primer verbo; usarlo en `Session::query/execute` cuando `spec.read_only`.
- [ ] **Step 3: CI** `cargo test --workspace` y `cargo clippy --workspace --all-targets`; la integración Oracle queda fuera de CI público y documentada.
- [ ] **Step 4: Verde + commit + push.**

## Cierre de P1a

- [ ] `cargo test --workspace` verde en Windows (unit) y CI en 3 SO.
- [ ] Integración Oracle real: dual, tipos comunes, cancelación y 100k filas por lotes.
- [ ] `sqlt-cli query/bench` funcionando contra `santa` o el alias elegido.
- [ ] Sin credenciales en el repo; `.env.local` ignorado.
