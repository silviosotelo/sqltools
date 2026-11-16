// SPDX-License-Identifier: GPL-3.0-only
// Paridad con los formatos soportados por src/Common/FastDateFormat.cpp
// (validados contra TO_CHAR de Oracle en etapa1/sqlt_datefmt_test.cpp).

use sqlt_core::datefmt::compile_date_format;

const FEB29: (i32, u32, u32, u32, u32, u32) = (2024, 2, 29, 13, 45, 9);
const MAR1_1900: (i32, u32, u32, u32, u32, u32) = (1900, 3, 1, 0, 0, 0);
const JAN1_2026: (i32, u32, u32, u32, u32, u32) = (2026, 1, 1, 23, 59, 59);

fn fmt(f: &str, dt: (i32, u32, u32, u32, u32, u32)) -> String {
    let df = compile_date_format(f).unwrap_or_else(|| panic!("formato no soportado: {f}"));
    df.format(dt.0, dt.1, dt.2, dt.3, dt.4, dt.5)
}

#[test]
fn original_date_formats() {
    assert_eq!(fmt("DD-MON-RR", FEB29), "29-FEB-24");
    assert_eq!(fmt("DD-MON-YYYY", FEB29), "29-FEB-2024");
    assert_eq!(fmt("YYYY-MM-DD HH24:MI:SS", FEB29), "2024-02-29 13:45:09");
    assert_eq!(fmt("YYYY-MM-DD", FEB29), "2024-02-29");
    assert_eq!(fmt("DD/MM/YYYY", FEB29), "29/02/2024");
    assert_eq!(fmt("DD/MM/YYYY HH24:MI:SS", FEB29), "29/02/2024 13:45:09");
    assert_eq!(fmt("DD-MON-RR HH.MI.SS AM", FEB29), "29-FEB-24 01.45.09 PM");
    assert_eq!(
        fmt("MM/DD/YYYY HH:MI:SS AM", FEB29),
        "02/29/2024 01:45:09 PM"
    );
    assert_eq!(
        fmt("Day, Month DD, YYYY", FEB29),
        "Thursday , February  29, 2024"
    );
    assert_eq!(fmt("DY DD Mon YYYY", FEB29), "THU 29 Feb 2024");
    assert_eq!(fmt("DAY DD-MON-YY", FEB29), "THURSDAY  29-FEB-24");
    assert_eq!(
        fmt("YYYY-MM-DD\"T\"HH24:MI:SS", FEB29),
        "2024-02-29T13:45:09"
    );
    assert_eq!(fmt("DDD YYYY Q", FEB29), "060 2024 1");
}

#[test]
fn leap_year_and_century_start() {
    assert_eq!(fmt("DD-MON-YYYY", MAR1_1900), "01-MAR-1900");
    assert_eq!(
        fmt("YYYY-MM-DD HH24:MI:SS", MAR1_1900),
        "1900-03-01 00:00:00"
    );
    assert_eq!(
        fmt("DD-MON-RR HH.MI.SS AM", MAR1_1900),
        "01-MAR-00 12.00.00 AM"
    );
    assert_eq!(fmt("DDD YYYY Q", MAR1_1900), "060 1900 1");
}

#[test]
fn midnight_and_noon() {
    assert_eq!(
        fmt("DD/MM/YYYY HH24:MI:SS", JAN1_2026),
        "01/01/2026 23:59:59"
    );
    assert_eq!(
        fmt("MM/DD/YYYY HH:MI:SS AM", JAN1_2026),
        "01/01/2026 11:59:59 PM"
    );
    assert_eq!(fmt("DD-MON-RR", JAN1_2026), "01-JAN-26");
    let noon = (2024, 2, 29, 12, 0, 0);
    assert_eq!(fmt("HH24:MI:SS AM", noon), "12:00:00 PM");
    let midnight = (2024, 2, 29, 0, 0, 0);
    assert_eq!(fmt("HH12:MI:SS AM", midnight), "12:00:00 AM");
}

#[test]
fn months_and_days_in_three_cases() {
    assert_eq!(fmt("MON", FEB29), "FEB");
    assert_eq!(fmt("Mon", FEB29), "Feb");
    assert_eq!(fmt("mon", FEB29), "feb");
    assert_eq!(fmt("MONTH", FEB29), "FEBRUARY ");
    assert_eq!(fmt("Month", FEB29), "February ");
    assert_eq!(fmt("month", FEB29), "february ");
    assert_eq!(fmt("DAY", FEB29), "THURSDAY ");
    assert_eq!(fmt("Day", FEB29), "Thursday ");
    assert_eq!(fmt("day", FEB29), "thursday ");
}

#[test]
fn unsupported_formats_return_none() {
    assert!(compile_date_format("dd-mon-rr").is_none());
    assert!(compile_date_format("FMDD-MON-YY").is_none());
    assert!(compile_date_format("DD-MON-RR HH24:MI:SSXFF").is_none());
    assert!(compile_date_format("").is_none());
    assert!(compile_date_format("\"sin cerrar").is_none());
}
