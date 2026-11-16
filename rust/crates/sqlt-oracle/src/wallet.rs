// SPDX-License-Identifier: GPL-3.0-only
//! Cross-platform wallet/TLS resolution, ported from the C++/Qt port.
//!
//! Windows `tnsnames.ora` files often point to wallets with `C:\...` paths.
//! Autonomous Database wallets usually have `DIRECTORY="?/network/admin"` in
//! `sqlnet.ora`, which is meaningless for Instant Client. When needed, a
//! temporary TNS directory is prepared with the corrected configuration; the
//! user's files are never modified.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalletPlan {
    pub tns_admin: PathBuf,
    pub wallet_dir: Option<PathBuf>,
    pub overlay: bool,
    pub note: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AliasCheck {
    pub alias: String,
    pub tls: bool,
    pub wallet_path: Option<PathBuf>,
    pub ok: bool,
    pub message: String,
}

fn read_text(path: &Path) -> Option<String> {
    fs::read(path)
        .ok()
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
}

fn contains_ci(haystack: &str, needle: &str) -> bool {
    haystack
        .to_ascii_lowercase()
        .contains(&needle.to_ascii_lowercase())
}

fn tnsnames_path(folder: &Path) -> Option<PathBuf> {
    for name in ["tnsnames.ora", "TNSNAMES.ORA"] {
        let path = folder.join(name);
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

pub fn has_wallet(dir: &Path) -> bool {
    dir.is_dir() && dir.join("cwallet.sso").is_file()
}

fn child_dirs(dir: &Path) -> Vec<PathBuf> {
    fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(|entry| entry.ok())
                .map(|entry| entry.path())
                .filter(|path| path.is_dir())
                .collect()
        })
        .unwrap_or_default()
}

/// Finds a wallet by the leaf name used by the TNS (folder name or `.zip`),
/// scanning the given roots up to two levels deep.
fn find_wallet_by_name(leaf: &str, roots: &[PathBuf]) -> Option<PathBuf> {
    if leaf.is_empty() {
        return None;
    }
    for root in roots {
        if !root.is_dir() {
            continue;
        }
        let mut dirs = vec![root.clone()];
        for d1 in child_dirs(root) {
            dirs.push(d1.clone());
            for d2 in child_dirs(&d1) {
                dirs.push(d2);
            }
        }
        for dir in dirs {
            let name = dir.file_name()?.to_string_lossy();
            if name.eq_ignore_ascii_case(leaf) && has_wallet(&dir) {
                return Some(dir);
            }
            if has_wallet(&dir) && dir.join(format!("{leaf}.zip")).is_file() {
                return Some(dir);
            }
        }
    }
    None
}

pub fn alias_names(folder: &Path) -> Vec<String> {
    let Some(text) = tnsnames_path(folder).and_then(|path| read_text(&path)) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        let Some((name, _)) = trimmed.split_once('=') else {
            continue;
        };
        let name = name.trim();
        let mut chars = name.chars();
        let Some(first) = chars.next() else {
            continue;
        };
        if !first.is_ascii_alphabetic() {
            continue;
        }
        if chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-')) {
            out.push(name.to_string());
        }
    }
    out
}

pub fn alias_block(text: &str, alias: &str) -> String {
    let mut offset = 0usize;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim_start();
        let lead = line.len() - trimmed.len();
        if let Some(eq) = trimmed.find('=') {
            let name = trimmed[..eq].trim();
            if name.eq_ignore_ascii_case(alias) {
                let after_eq = offset + lead + eq + 1;
                let rest = &text[after_eq..];
                let start = match rest.find('(') {
                    Some(pos) => after_eq + pos,
                    None => return rest.trim().to_string(),
                };
                let mut depth = 0i32;
                let mut end = start;
                for (i, ch) in text[start..].char_indices() {
                    match ch {
                        '(' => depth += 1,
                        ')' => {
                            depth -= 1;
                            if depth == 0 {
                                end = start + i + 1;
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                if end > start {
                    return text[start..end].to_string();
                }
                return rest.trim().to_string();
            }
        }
        offset += line.len();
    }
    String::new()
}

fn my_wallet_directory(block: &str) -> Option<String> {
    let lower = block.to_ascii_lowercase();
    let key = "my_wallet_directory";
    let mut from = 0usize;
    while let Some(pos) = lower[from..].find(key) {
        let abs = from + pos + key.len();
        let rest = &block[abs..];
        if let Some(eq) = rest.find('=') {
            let after = &rest[eq + 1..];
            let end = after.find([')', '\n']).unwrap_or(after.len());
            let value = after[..end].trim().replace('"', "");
            if !value.is_empty() {
                return Some(value);
            }
        }
        from = abs;
    }
    None
}

fn sqlnet_wallet_dir(text: &str) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    let wallet_pos = lower.find("wallet_location")?;
    let rest = &text[wallet_pos..];
    let dir_pos = rest.to_ascii_lowercase().find("directory")?;
    let after_dir = &rest[dir_pos + "directory".len()..];
    let eq = after_dir.find('=')?;
    let mut value = after_dir[eq + 1..].trim_start();
    value = value.strip_prefix('"').unwrap_or(value);
    let end = value.find(['"', ')', '\n']).unwrap_or(value.len());
    let value = value[..end].trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn uses_tls(block: &str) -> bool {
    let lower = block.to_ascii_lowercase();
    let Some(protocol) = lower.find("protocol") else {
        return false;
    };
    let after = &lower[protocol + "protocol".len()..];
    let Some(eq) = after.find('=') else {
        return false;
    };
    after[eq + 1..].trim_start().starts_with("tcps")
}

/// `C:\Mac\Home\x` (a Parallels shared home) becomes `<home>/x`. Other Windows
/// paths are returned unchanged; on Unix they simply won't exist.
pub fn map_windows_path(path: &str, home: &Path) -> PathBuf {
    let trimmed = path.trim();
    let bytes = trimmed.as_bytes();
    if bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        if bytes[2] != b'\\' && bytes[2] != b'/' {
            return PathBuf::from(trimmed);
        }
        let rest = trimmed[3..].replace('\\', "/");
        let lower = rest.to_ascii_lowercase();
        if lower == "mac/home" || lower.starts_with("mac/home/") {
            let suffix = rest[8..].trim_start_matches('/');
            return if suffix.is_empty() {
                home.to_path_buf()
            } else {
                home.join(suffix)
            };
        }
    }
    PathBuf::from(trimmed)
}

fn same_path(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

fn short_hash(value: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in value.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{:012x}", hash & 0xffffffffffff)
}

fn link_or_copy(source: &Path, target: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        if std::os::unix::fs::symlink(source, target).is_ok() {
            return Ok(());
        }
    }
    #[cfg(windows)]
    {
        if std::os::windows::fs::symlink_file(source, target).is_ok() {
            return Ok(());
        }
    }
    fs::copy(source, target).map(|_| ())
}

fn selection_error(alias: &str, tns_path: &str, mac_path: &Path) -> String {
    let translated = if mac_path.to_string_lossy() != tns_path {
        format!(" (on this machine: \"{}\")", mac_path.to_string_lossy())
    } else {
        String::new()
    };
    format!(
        "Alias \"{alias}\" uses TLS and its wallet was not found.\n\
         The TNS points to \"{tns_path}\"{translated}, which does not exist here.\n\
         Set the wallet folder (the one containing cwallet.sso) as the connection's \"TNS / wallet folder\"."
    )
}

pub fn prepare(tns_folder: &Path, alias: &str, overlay_root: &Path, home: &Path) -> WalletPlan {
    let mut plan = WalletPlan {
        tns_admin: tns_folder.to_path_buf(),
        wallet_dir: None,
        overlay: false,
        note: String::new(),
        error: None,
    };
    if tns_folder.as_os_str().is_empty() {
        return plan;
    }
    let Some(tns_file) = tnsnames_path(tns_folder) else {
        return plan;
    };
    let tns_text = read_text(&tns_file).unwrap_or_default();
    let block = alias_block(&tns_text, alias);
    if block.is_empty() {
        return plan;
    }

    let tls = uses_tls(&block);
    let my_wallet = my_wallet_directory(&block).unwrap_or_default();
    let my_wallet_mac = if my_wallet.is_empty() {
        PathBuf::new()
    } else {
        map_windows_path(&my_wallet, home)
    };
    let sqlnet_text = read_text(&tns_folder.join("sqlnet.ora")).unwrap_or_default();
    let sqlnet_dir = sqlnet_wallet_dir(&sqlnet_text).unwrap_or_default();

    let mut wallet: Option<PathBuf> = None;
    if !my_wallet_mac.as_os_str().is_empty() && has_wallet(&my_wallet_mac) {
        wallet = Some(my_wallet_mac.clone());
    }
    if wallet.is_none() {
        let path = PathBuf::from(&sqlnet_dir);
        if path.is_absolute() && has_wallet(&path) {
            wallet = Some(path);
        }
    }
    if wallet.is_none() && has_wallet(tns_folder) {
        wallet = Some(tns_folder.to_path_buf());
    }
    if wallet.is_none() && !my_wallet.is_empty() {
        let leaf = my_wallet
            .rsplit(['\\', '/'])
            .next()
            .unwrap_or_default()
            .to_string();
        let parent = tns_folder
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default();
        wallet = find_wallet_by_name(&leaf, &[tns_folder.to_path_buf(), parent]);
    }
    plan.wallet_dir = wallet.clone();

    if !tls && my_wallet.is_empty() {
        return plan;
    }
    let Some(wallet) = wallet else {
        if tls {
            plan.error = Some(selection_error(alias, &my_wallet, &my_wallet_mac));
        }
        return plan;
    };

    let tns_ok = my_wallet.is_empty()
        || (my_wallet == my_wallet_mac.to_string_lossy()
            && same_path(Path::new(&my_wallet), &wallet));
    let sqlnet_ok = !sqlnet_dir.is_empty()
        && Path::new(&sqlnet_dir).is_absolute()
        && same_path(Path::new(&sqlnet_dir), &wallet);
    if tns_ok && (sqlnet_ok || !my_wallet.is_empty()) {
        return plan;
    }

    let key = short_hash(&format!("{}|{}", tns_folder.display(), alias));
    let dir = overlay_root.join(key);
    let _ = fs::remove_dir_all(&dir);
    if let Err(err) = fs::create_dir_all(&dir) {
        plan.error = Some(format!(
            "Could not prepare the temporary configuration: {err}"
        ));
        return plan;
    }
    if let Ok(entries) = fs::read_dir(tns_folder) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            if name == "tnsnames.ora" || name == "sqlnet.ora" {
                continue;
            }
            if let Err(err) = link_or_copy(&path, &dir.join(entry.file_name())) {
                plan.error = Some(format!("Could not link {}: {err}", path.display()));
                return plan;
            }
        }
    }

    let new_tns = if my_wallet.is_empty() {
        tns_text
    } else {
        tns_text.replace(&my_wallet, &wallet.display().to_string())
    };
    if let Err(err) = fs::write(dir.join("tnsnames.ora"), new_tns) {
        plan.error = Some(format!("Could not prepare temporary tnsnames.ora: {err}"));
        return plan;
    }

    let kept: Vec<&str> = sqlnet_text
        .lines()
        .filter(|line| !contains_ci(line, "wallet_location"))
        .collect();
    let sqlnet = format!(
        "{}\nWALLET_LOCATION = (SOURCE = (METHOD = file) (METHOD_DATA = (DIRECTORY=\"{}\")))\n",
        kept.join("\n").trim(),
        wallet.display()
    );
    if let Err(err) = fs::write(dir.join("sqlnet.ora"), sqlnet) {
        plan.error = Some(format!("Could not prepare temporary sqlnet.ora: {err}"));
        return plan;
    }

    plan.tns_admin = dir;
    plan.overlay = true;
    plan.note = format!(
        "Wallet: {} (temporary TNS configuration; user files are not modified)",
        wallet.display()
    );
    plan
}

pub fn inspect(tns_folder: &Path, home: &Path) -> Vec<AliasCheck> {
    let text = tnsnames_path(tns_folder)
        .and_then(|path| read_text(&path))
        .unwrap_or_default();
    let sqlnet_dir =
        sqlnet_wallet_dir(&read_text(&tns_folder.join("sqlnet.ora")).unwrap_or_default())
            .unwrap_or_default();
    let mut out = Vec::new();
    for alias in alias_names(tns_folder) {
        let block = alias_block(&text, &alias);
        let tls = uses_tls(&block);
        let my_wallet = my_wallet_directory(&block).unwrap_or_default();
        let my_wallet_mac = if my_wallet.is_empty() {
            PathBuf::new()
        } else {
            map_windows_path(&my_wallet, home)
        };
        let mut check = AliasCheck {
            alias: alias.clone(),
            tls,
            wallet_path: None,
            ok: true,
            message: String::new(),
        };
        if !tls && my_wallet.is_empty() {
            check.message = "No TLS: wallet not needed.".to_string();
            out.push(check);
            continue;
        }
        if !my_wallet.is_empty() {
            if my_wallet == my_wallet_mac.to_string_lossy() && has_wallet(&my_wallet_mac) {
                check.wallet_path = Some(my_wallet_mac.clone());
                check.message = format!("Wallet from TNS: {}", my_wallet_mac.display());
            } else if my_wallet != my_wallet_mac.to_string_lossy() && has_wallet(&my_wallet_mac) {
                check.ok = false;
                check.wallet_path = Some(my_wallet_mac.clone());
                check.message = format!(
                    "The TNS uses the Windows path \"{my_wallet}\". On this machine it is at \"{}\": fix the path in tnsnames.ora.",
                    my_wallet_mac.display()
                );
            } else {
                check.ok = false;
                let leaf = my_wallet_mac
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let parent = tns_folder
                    .parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_default();
                let guess = find_wallet_by_name(&leaf, &[tns_folder.to_path_buf(), parent]);
                check.message = match &guess {
                    Some(path) => format!(
                        "The TNS points to \"{my_wallet}\", which does not exist on this machine. The wallet appears to be at \"{}\".",
                        path.display()
                    ),
                    None => format!(
                        "The TNS points to \"{my_wallet}\", which does not exist on this machine. No wallet with that name was found."
                    ),
                };
                check.wallet_path = guess;
            }
        } else if !sqlnet_dir.is_empty()
            && Path::new(&sqlnet_dir).is_absolute()
            && has_wallet(Path::new(&sqlnet_dir))
        {
            check.wallet_path = Some(PathBuf::from(&sqlnet_dir));
            check.message = format!("Wallet from sqlnet.ora: {sqlnet_dir}");
        } else if has_wallet(tns_folder) {
            check.wallet_path = Some(tns_folder.to_path_buf());
            check.message =
                "Wallet in the TNS folder itself (sqlnet.ora does not point to a valid path)."
                    .to_string();
        } else {
            check.ok = false;
            check.message =
                "Uses TLS and the wallet was not found: add (my_wallet_directory= /path/to/wallet) to the entry."
                    .to_string();
        }
        out.push(check);
    }
    out
}
