// SPDX-License-Identifier: GPL-3.0-only
use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;

use sqlt_oracle::oci::locate_oci_dir_in;

fn make_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sqlt-oci-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let lib = if cfg!(windows) {
        "oci.dll"
    } else {
        "libclntsh.dylib"
    };
    fs::write(dir.join(lib), b"fake").unwrap();
    dir
}

fn cleanup(dirs: &[&PathBuf]) {
    for dir in dirs {
        let _ = fs::remove_dir_all(dir);
    }
}

#[test]
fn dpi_has_priority_over_sqlt_and_path() {
    let dpi = make_dir("dpi");
    let sqlt = make_dir("sqlt");
    let path_dir = make_dir("path");
    let path_env: OsString = std::env::join_paths([&path_dir]).unwrap();
    let got = locate_oci_dir_in(
        Some(&dpi),
        Some(&sqlt),
        Some(path_env.as_os_str()),
        std::slice::from_ref(&path_dir),
    );
    assert_eq!(got, Some(dpi.clone()));
    cleanup(&[&dpi, &sqlt, &path_dir]);
}

#[test]
fn falls_back_to_sqlt_without_dpi() {
    let sqlt = make_dir("solo-sqlt");
    let got = locate_oci_dir_in(None, Some(&sqlt), None, &[]);
    assert_eq!(got, Some(sqlt.clone()));
    cleanup(&[&sqlt]);
}

#[test]
fn falls_back_to_path_then_candidates() {
    let path_dir = make_dir("path2");
    let path_env = std::env::join_paths([&path_dir]).unwrap();
    assert_eq!(
        locate_oci_dir_in(None, None, Some(path_env.as_os_str()), &[]),
        Some(path_dir.clone())
    );
    let candidate = make_dir("candidate");
    assert_eq!(
        locate_oci_dir_in(None, None, None, std::slice::from_ref(&candidate)),
        Some(candidate.clone())
    );
    cleanup(&[&path_dir, &candidate]);
}

#[test]
fn no_client_returns_none() {
    let empty = std::env::temp_dir().join(format!("sqlt-oci-empty-{}", std::process::id()));
    let _ = fs::remove_dir_all(&empty);
    fs::create_dir_all(&empty).unwrap();
    assert_eq!(
        locate_oci_dir_in(None, None, None, std::slice::from_ref(&empty)),
        None
    );
    cleanup(&[&empty]);
}
