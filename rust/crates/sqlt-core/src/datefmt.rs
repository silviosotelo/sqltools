// SPDX-License-Identifier: GPL-3.0-only
//! Formateador rápido de fechas Oracle, port de `src/Common/FastDateFormat.cpp`.
//!
//! Solo cubre los elementos comunes en inglés. Si el formato usa algo no
//! soportado, [`compile_date_format`] devuelve `None` y el llamador debe usar
//! el formateo del driver (más lento, mismo resultado).

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

const DAYS: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];

#[derive(Debug, Clone, PartialEq, Eq)]
enum Item {
    Literal(String),
    Y4,
    Y3,
    Y2,
    MM,
    MonU,
    MonC,
    MonL,
    MonthU,
    MonthC,
    MonthL,
    DD,
    Ddd,
    HH12,
    HH24,
    MI,
    SS,
    AmPm,
    DyU,
    DyC,
    DyL,
    DayU,
    DayC,
    DayL,
    Q,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DateFormat {
    items: Vec<Item>,
}

fn starts_with_ci(s: &str, tok: &str) -> bool {
    let b = s.as_bytes();
    let t = tok.as_bytes();
    b.len() >= t.len()
        && b[..t.len()]
            .iter()
            .zip(t)
            .all(|(a, b)| a.eq_ignore_ascii_case(b))
}

/// 0 = MAYÚSCULAS, 1 = Capitalizado, 2 = minúsculas, -1 = mixto.
fn case_of(s: &str, n: usize) -> i32 {
    let b = &s.as_bytes()[..n];
    if b.iter().all(|c| c.is_ascii_uppercase()) {
        0
    } else if b.iter().all(|c| c.is_ascii_lowercase()) {
        2
    } else if n > 1 && b[0].is_ascii_uppercase() && b[1..].iter().all(|c| c.is_ascii_lowercase()) {
        1
    } else {
        -1
    }
}

/// Compila un formato NLS. `None` = no soportado por el camino rápido.
pub fn compile_date_format(fmt: &str) -> Option<DateFormat> {
    if fmt.is_empty() {
        return None;
    }
    let mut items = Vec::new();
    let mut lit = String::new();
    let mut i = 0usize;
    let bytes = fmt.as_bytes();
    let n = bytes.len();

    while i < n {
        let c = bytes[i];
        if c == b'"' {
            let rest = &fmt[i + 1..];
            let close = rest.find('"')?;
            lit.push_str(&rest[..close]);
            i += close + 2;
            continue;
        }
        if !c.is_ascii_alphabetic() {
            if matches!(c, b'-' | b'/' | b',' | b'.' | b':' | b';' | b' ') {
                lit.push(c as char);
                i += 1;
                continue;
            }
            return None;
        }
        if !lit.is_empty() {
            items.push(Item::Literal(std::mem::take(&mut lit)));
        }

        let rest = &fmt[i..];
        let take_case = |tok: &str, u: Item, cc: Item, l: Item| -> Option<Item> {
            if !starts_with_ci(rest, tok) {
                return None;
            }
            let cs = case_of(rest, tok.len());
            match cs {
                0 => Some(u),
                1 => Some(cc),
                2 => Some(l),
                _ => None,
            }
        };

        if (starts_with_ci(rest, "YYYY") && case_of(rest, 4) == 0)
            || (starts_with_ci(rest, "RRRR") && case_of(rest, 4) == 0)
        {
            items.push(Item::Y4);
            i += 4;
            continue;
        }
        if starts_with_ci(rest, "YYY") && case_of(rest, 3) == 0 {
            items.push(Item::Y3);
            i += 3;
            continue;
        }
        if (starts_with_ci(rest, "YY") && case_of(rest, 2) == 0)
            || (starts_with_ci(rest, "RR") && case_of(rest, 2) == 0)
        {
            items.push(Item::Y2);
            i += 2;
            continue;
        }
        if starts_with_ci(rest, "MONTH") {
            let item = take_case("MONTH", Item::MonthU, Item::MonthC, Item::MonthL)?;
            items.push(item);
            i += 5;
            continue;
        }
        if starts_with_ci(rest, "MON") {
            let item = take_case("MON", Item::MonU, Item::MonC, Item::MonL)?;
            items.push(item);
            i += 3;
            continue;
        }
        if starts_with_ci(rest, "DAY") {
            let item = take_case("DAY", Item::DayU, Item::DayC, Item::DayL)?;
            items.push(item);
            i += 3;
            continue;
        }
        if starts_with_ci(rest, "DY") {
            let item = take_case("DY", Item::DyU, Item::DyC, Item::DyL)?;
            items.push(item);
            i += 2;
            continue;
        }
        if starts_with_ci(rest, "Ddd") && case_of(rest, 3) == 0 {
            items.push(Item::Ddd);
            i += 3;
            continue;
        }
        // Los tokens numéricos exigen MAYÚSCULAS exactas (como el original).
        if rest.starts_with("MM") {
            items.push(Item::MM);
            i += 2;
            continue;
        }
        if rest.starts_with("DD") {
            items.push(Item::DD);
            i += 2;
            continue;
        }
        if rest.starts_with("HH24") {
            items.push(Item::HH24);
            i += 4;
            continue;
        }
        if rest.starts_with("HH12") {
            items.push(Item::HH12);
            i += 4;
            continue;
        }
        if rest.starts_with("HH") {
            items.push(Item::HH12);
            i += 2;
            continue;
        }
        if rest.starts_with("MI") {
            items.push(Item::MI);
            i += 2;
            continue;
        }
        if rest.starts_with("SS") {
            items.push(Item::SS);
            i += 2;
            continue;
        }
        if rest.starts_with("AM") || rest.starts_with("PM") {
            items.push(Item::AmPm);
            i += 2;
            continue;
        }
        if c == b'Q' {
            items.push(Item::Q);
            i += 1;
            continue;
        }
        return None;
    }

    if !lit.is_empty() {
        items.push(Item::Literal(lit));
    }
    if items.is_empty() {
        None
    } else {
        Some(DateFormat { items })
    }
}

fn day_of_week(y: i32, m: u32, d: u32) -> usize {
    const T: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let mut y = y;
    if m < 3 {
        y -= 1;
    }
    let idx = (m.clamp(1, 12) - 1) as usize;
    ((y + y / 4 - y / 100 + y / 400 + T[idx] + d as i32).rem_euclid(7)) as usize
}

fn day_of_year(y: i32, m: u32, d: u32) -> u32 {
    const CUM: [u32; 12] = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    let idx = (m.clamp(1, 12) - 1) as usize;
    let mut r = CUM[idx] + d;
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    if leap && m > 2 {
        r += 1;
    }
    r
}

fn append_case(out: &mut String, s: &str, cs: i32) {
    for c in s.chars() {
        if cs == 0 {
            out.push(c.to_ascii_uppercase());
        } else if cs == 2 {
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
}

impl DateFormat {
    pub fn format(&self, y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> String {
        let month_idx = (mo.clamp(1, 12) - 1) as usize;
        let mut out = String::new();
        let mut dow: Option<usize> = None;
        for item in &self.items {
            match item {
                Item::Literal(t) => out.push_str(t),
                Item::Y4 => out.push_str(&format!("{y:04}")),
                Item::Y3 => out.push_str(&format!("{:03}", y.rem_euclid(1000))),
                Item::Y2 => out.push_str(&format!("{:02}", y.rem_euclid(100))),
                Item::MM => out.push_str(&format!("{mo:02}")),
                Item::DD => out.push_str(&format!("{d:02}")),
                Item::Ddd => out.push_str(&format!("{:03}", day_of_year(y, mo, d))),
                Item::HH24 => out.push_str(&format!("{h:02}")),
                Item::HH12 => {
                    let h12 = if h.is_multiple_of(12) { 12 } else { h % 12 };
                    out.push_str(&format!("{h12:02}"));
                }
                Item::MI => out.push_str(&format!("{mi:02}")),
                Item::SS => out.push_str(&format!("{s:02}")),
                Item::AmPm => out.push_str(if h < 12 { "AM" } else { "PM" }),
                Item::Q => out.push(char::from(b'0' + ((mo.clamp(1, 12) - 1) / 3 + 1) as u8)),
                Item::MonU | Item::MonC | Item::MonL => {
                    let name = MONTHS[month_idx];
                    append_case(
                        &mut out,
                        &name[..3],
                        if matches!(item, Item::MonU) {
                            0
                        } else if matches!(item, Item::MonC) {
                            1
                        } else {
                            2
                        },
                    );
                }
                Item::MonthU | Item::MonthC | Item::MonthL => {
                    let mut name = MONTHS[month_idx].to_string();
                    while name.len() < 9 {
                        name.push(' ');
                    }
                    append_case(
                        &mut out,
                        &name,
                        if matches!(item, Item::MonthU) {
                            0
                        } else if matches!(item, Item::MonthC) {
                            1
                        } else {
                            2
                        },
                    );
                }
                Item::DyU | Item::DyC | Item::DyL => {
                    let i = *dow.get_or_insert_with(|| day_of_week(y, mo, d));
                    append_case(
                        &mut out,
                        &DAYS[i][..3],
                        if matches!(item, Item::DyU) {
                            0
                        } else if matches!(item, Item::DyC) {
                            1
                        } else {
                            2
                        },
                    );
                }
                Item::DayU | Item::DayC | Item::DayL => {
                    let i = *dow.get_or_insert_with(|| day_of_week(y, mo, d));
                    let mut name = DAYS[i].to_string();
                    while name.len() < 9 {
                        name.push(' ');
                    }
                    append_case(
                        &mut out,
                        &name,
                        if matches!(item, Item::DayU) {
                            0
                        } else if matches!(item, Item::DayC) {
                            1
                        } else {
                            2
                        },
                    );
                }
            }
        }
        out
    }
}

/// Solo es seguro usar el formateador propio si el cliente Oracle está en
/// inglés (nombres de mes y día).
pub fn language_is_english(date_language: Option<&str>, nls_lang: Option<&str>) -> bool {
    fn ok(v: Option<&str>) -> bool {
        match v {
            None => true,
            Some(s) => {
                s.is_empty()
                    || s.to_ascii_uppercase().starts_with("AMERICAN")
                    || s.to_ascii_uppercase().starts_with("ENGLISH")
            }
        }
    }
    ok(date_language) && ok(nls_lang)
}

pub fn client_language_is_english() -> bool {
    let dl = std::env::var("NLS_DATE_LANGUAGE").ok();
    let lang = std::env::var("NLS_LANG").ok();
    language_is_english(dl.as_deref(), lang.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lenguaje_ingles() {
        assert!(language_is_english(None, None));
        assert!(language_is_english(
            Some("AMERICAN"),
            Some("AMERICAN_AMERICA.AL32UTF8")
        ));
        assert!(!language_is_english(Some("SPANISH"), None));
        assert!(!language_is_english(
            None,
            Some("SPANISH_SPAIN.WE8ISO8859P1")
        ));
    }
}
