// SPDX-License-Identifier: GPL-3.0-only
// Ported from tests/wallet_test.cpp: temporary folders imitate ADB wallets and
// Windows TNS files. No Oracle connection is needed.

use std::fs;
use std::path::{Path, PathBuf};

use sqlt_oracle::wallet::{alias_names, inspect, map_windows_path, prepare, AliasCheck};

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Fixture {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("sqlt-wallet-{}-{name}-{nanos}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        Fixture { root }
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    fn put(&self, rel: &str, text: &str) {
        let path = self.path(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, text).unwrap();
    }

    fn read(&self, rel: &str) -> String {
        fs::read_to_string(self.path(rel)).unwrap_or_default()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn tls_entry(alias: &str, extra: &str) -> String {
    format!(
        "{alias} = (description= (retry_count=20)(retry_delay=3)\
         (address=(protocol=tcps)(port=1522)(host=h.example.com))\
         (connect_data=(service_name=svc))\
         (security=(ssl_server_dn_match=yes){extra}))\n"
    )
}

fn by<'a>(checks: &'a [AliasCheck], alias: &str) -> &'a AliasCheck {
    checks
        .iter()
        .find(|check| check.alias == alias)
        .unwrap_or_else(|| panic!("alias {alias} not found"))
}

#[test]
fn wallet_resolution_cases() {
    let fixture = Fixture::new("all");
    let home = fixture.path("home");
    let overlay = fixture.path("overlay");
    fs::create_dir_all(&home).unwrap();

    // 1. Standard ADB wallet with "?/network/admin": overlay required.
    fixture.put("w1/tnsnames.ora", &tls_entry("db_high", ""));
    fixture.put("w1/cwallet.sso", "x");
    fixture.put(
        "w1/sqlnet.ora",
        "WALLET_LOCATION = (SOURCE = (METHOD = file) (METHOD_DATA = (DIRECTORY=\"?/network/admin\")))\nSSL_SERVER_DN_MATCH=yes\n",
    );
    let plan = prepare(&fixture.path("w1"), "DB_HIGH", &overlay, &home);
    assert!(plan.overlay && plan.error.is_none());
    assert_eq!(plan.wallet_dir, Some(fixture.path("w1")));
    assert!(plan.tns_admin.starts_with(&overlay));
    let sqlnet = fs::read_to_string(plan.tns_admin.join("sqlnet.ora")).unwrap();
    assert!(sqlnet.contains(&format!("DIRECTORY=\"{}\"", fixture.path("w1").display())));
    assert!(sqlnet.contains("SSL_SERVER_DN_MATCH"));
    assert!(!sqlnet.contains("?/network"));
    assert!(plan.tns_admin.join("cwallet.sso").is_file());
    assert!(fixture.read("w1/sqlnet.ora").contains("?/network/admin"));

    // 2. sqlnet.ora already correct: use it as is.
    fixture.put("w2/tnsnames.ora", &tls_entry("db_high", ""));
    fixture.put("w2/cwallet.sso", "x");
    fixture.put(
        "w2/sqlnet.ora",
        &format!(
            "WALLET_LOCATION = (SOURCE = (METHOD = file) (METHOD_DATA = (DIRECTORY=\"{}\")))\n",
            fixture.path("w2").display()
        ),
    );
    let plan = prepare(&fixture.path("w2"), "db_high", &overlay, &home);
    assert!(!plan.overlay && plan.error.is_none());
    assert_eq!(plan.tns_admin, fixture.path("w2"));

    // 3. Windows path, wallet in the TNS folder: rewrite the path in an overlay.
    fixture.put(
        "w3/tnsnames.ora",
        &tls_entry(
            "db_high",
            "(my_wallet_directory= C:\\Wallet_SqltTest_NoSuch)",
        ),
    );
    fixture.put("w3/cwallet.sso", "x");
    let plan = prepare(&fixture.path("w3"), "db_high", &overlay, &home);
    assert!(plan.overlay);
    let tns = fs::read_to_string(plan.tns_admin.join("tnsnames.ora")).unwrap();
    assert!(tns.contains(&format!(
        "my_wallet_directory= {}",
        fixture.path("w3").display()
    )));
    assert!(!tns.contains("C:\\Wallet"));
    assert!(fixture
        .read("w3/tnsnames.ora")
        .contains("C:\\Wallet_SqltTest_NoSuch"));

    // 4. C:\Mac\Home\... maps to the user's home and finds the wallet.
    fixture.put("home/Desktop/CBC/Wallet_x/cwallet.sso", "x");
    fixture.put(
        "w4/tnsnames.ora",
        &tls_entry(
            "cbc_high",
            "(my_wallet_directory= C:\\Mac\\Home\\Desktop\\CBC\\Wallet_x)",
        ),
    );
    let plan = prepare(&fixture.path("w4"), "cbc_high", &overlay, &home);
    assert_eq!(plan.wallet_dir, Some(home.join("Desktop/CBC/Wallet_x")));
    assert!(plan.error.is_none() && plan.overlay);
    assert_eq!(
        map_windows_path("C:\\Mac\\Home\\a\\b", Path::new("/Users/u")),
        PathBuf::from("/Users/u/a/b")
    );
    assert_eq!(
        map_windows_path("C:\\Wallet_X", Path::new("/Users/u")),
        PathBuf::from("C:\\Wallet_X")
    );

    // 5. TLS with no wallet: clear error naming the TNS path.
    fixture.put(
        "w5/tnsnames.ora",
        &tls_entry("db_high", "(my_wallet_directory= C:\\Wallet_Perdida)"),
    );
    let plan = prepare(&fixture.path("w5"), "db_high", &overlay, &home);
    let error = plan.error.unwrap_or_default();
    assert!(error.contains("C:\\Wallet_Perdida") && error.contains("cwallet.sso"));

    // 6. TCP alias: nothing to prepare.
    fixture.put(
        "w6/tnsnames.ora",
        "INV = (DESCRIPTION=(ADDRESS=(PROTOCOL=TCP)(HOST=10.0.0.5)(PORT=1521))(CONNECT_DATA=(SERVICE_NAME=inv)))\n",
    );
    let plan = prepare(&fixture.path("w6"), "inv", &overlay, &home);
    assert!(!plan.overlay && plan.error.is_none());
    assert_eq!(plan.tns_admin, fixture.path("w6"));
    assert_eq!(plan.wallet_dir, None);

    // 7. Unknown alias and empty folder.
    let plan = prepare(&fixture.path("w1"), "no_existe", &overlay, &home);
    assert!(plan.error.is_none() && !plan.overlay);
    assert_eq!(plan.tns_admin, fixture.path("w1"));
    let plan = prepare(Path::new(""), "x", &overlay, &home);
    assert!(plan.tns_admin.as_os_str().is_empty() && plan.error.is_none());

    // 8. Multiple aliases: only the requested block is rewritten.
    fixture.put(
        "w9/tnsnames.ora",
        &format!(
            "{}{}",
            tls_entry("a_high", "(my_wallet_directory= C:\\Wallet_A)"),
            tls_entry("b_high", "(my_wallet_directory= C:\\Wallet_B)")
        ),
    );
    fixture.put("w9/cwallet.sso", "x");
    let plan = prepare(&fixture.path("w9"), "b_high", &overlay, &home);
    let tns = fs::read_to_string(plan.tns_admin.join("tnsnames.ora")).unwrap();
    assert!(tns.contains("C:\\Wallet_A") && !tns.contains("C:\\Wallet_B"));

    // 9. Windows root TNS: find wallets by folder name or .zip.
    fixture.put(
        "raiz/tnsnames.ora",
        &format!(
            "{}{}{}",
            tls_entry("br_high", "(my_wallet_directory= C:\\Wallet_KMBR01)"),
            tls_entry("km_high", "(my_wallet_directory= C:\\Wallet_SqltTest_Km)"),
            tls_entry("otra_high", "(my_wallet_directory= C:\\Wallet_Otra)")
        ),
    );
    fixture.put("raiz/br01/Wallet_KMBR01/cwallet.sso", "x");
    fixture.put("raiz/principal/cwallet.sso", "x");
    fixture.put("raiz/principal/Wallet_SqltTest_Km.zip", "z");
    let plan = prepare(&fixture.path("raiz"), "br_high", &overlay, &home);
    assert_eq!(
        plan.wallet_dir,
        Some(fixture.path("raiz/br01/Wallet_KMBR01"))
    );
    assert!(plan.overlay && plan.error.is_none());
    let plan = prepare(&fixture.path("raiz"), "km_high", &overlay, &home);
    assert_eq!(plan.wallet_dir, Some(fixture.path("raiz/principal")));
    assert!(plan.error.is_none());
    let plan = prepare(&fixture.path("raiz"), "otra_high", &overlay, &home);
    let error = plan.error.unwrap_or_default();
    assert!(error.contains("Wallet_Otra"));
    assert!(error.contains('\n') && !error.contains("\\n"));

    // 10. inspect(): Mac paths, Windows paths and TCP; suggestions.
    fixture.put("mac/wallets/uno/cwallet.sso", "x");
    fixture.put("mac/wallets/dos/cwallet.sso", "x");
    fixture.put(
        "mac/tnsnames.ora",
        &format!(
            "{}{}{}{}INV = (DESCRIPTION=(ADDRESS=(PROTOCOL=TCP)(HOST=10.0.0.5)(PORT=1521))(CONNECT_DATA=(SERVICE_NAME=inv)))\n",
            tls_entry(
                "uno_high",
                &format!("(my_wallet_directory={})", fixture.path("mac/wallets/uno").display())
            ),
            tls_entry(
                "dos_high",
                &format!("(my_wallet_directory= {})", fixture.path("mac/wallets/dos").display())
            ),
            tls_entry("windows_high", "(my_wallet_directory= C:\\Wallet_Uno)"),
            tls_entry(
                "falta_high",
                &format!("(my_wallet_directory= {})", fixture.path("mac/wallets/no_existe").display())
            )
        ),
    );
    assert_eq!(alias_names(&fixture.path("mac")).len(), 5);
    let checks = inspect(&fixture.path("mac"), &home);
    assert_eq!(checks.len(), 5);
    assert!(by(&checks, "uno_high").ok);
    assert_eq!(
        by(&checks, "uno_high").wallet_path,
        Some(fixture.path("mac/wallets/uno"))
    );
    assert!(by(&checks, "dos_high").ok);
    assert_eq!(
        by(&checks, "dos_high").wallet_path,
        Some(fixture.path("mac/wallets/dos"))
    );
    assert!(!by(&checks, "windows_high").ok);
    assert!(by(&checks, "windows_high")
        .message
        .contains("C:\\Wallet_Uno"));
    assert!(!by(&checks, "falta_high").ok);
    assert!(by(&checks, "falta_high").message.contains("no_existe"));
    assert!(by(&checks, "INV").ok && !by(&checks, "INV").tls);

    fixture.put("mac/wallets/Wallet_Uno/cwallet.sso", "x");
    let checks = inspect(&fixture.path("mac"), &home);
    let expected = fixture
        .path("mac/wallets/Wallet_Uno")
        .display()
        .to_string()
        .replace('\\', "/");
    let message = by(&checks, "windows_high").message.replace('\\', "/");
    assert!(message.contains(&expected));

    let plan = prepare(&fixture.path("mac"), "uno_high", &overlay, &home);
    assert!(!plan.overlay && plan.error.is_none());
    assert_eq!(plan.tns_admin, fixture.path("mac"));
    assert_eq!(plan.wallet_dir, Some(fixture.path("mac/wallets/uno")));
}
