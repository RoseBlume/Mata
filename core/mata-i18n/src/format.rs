use crate::Locale;

/// Integer/decimal grouping in a documented subset of locales; ASCII digits.
/// This is *not* a CLDR/ICU replacement.
#[derive(Debug, Clone, Copy)]
struct Separators { grouping: &'static str, decimal: &'static str }
fn separators(locale: &Locale) -> Separators {
    match locale.language() {
        "fr" => Separators { grouping: "\u{202f}", decimal: "," },
        "ru" | "uk" | "sv" | "nb" | "nn" | "fi" | "pl" | "cs" | "sk" => Separators { grouping: "\u{a0}", decimal: "," },
        "de" | "es" | "it" | "nl" | "pt" | "tr" | "id" | "vi" => Separators { grouping: ".", decimal: "," },
        "hi" | "bn" => Separators { grouping: ",", decimal: "." },
        _ => Separators { grouping: ",", decimal: "." },
    }
}

/// Format a finite number with a fixed number of fractional digits (0..=12).
/// Returns the original Rust representation for non-finite numbers.
pub fn format_number(locale: &Locale, number: f64, precision: usize) -> String {
    if !number.is_finite() { return number.to_string(); }
    let prec = precision.min(12);
    let fixed = format!("{number:.prec$}");
    format_decimal(locale, &fixed)
}
/// Format an i64 using locale-specific grouping.
pub fn format_integer(locale: &Locale, number: i64) -> String {
    format_decimal(locale, &number.to_string())
}
/// Reformat a decimal already represented with a dot, preserving its precision.
/// Does not interpret scientific notation.
pub(crate) fn format_decimal(locale: &Locale, decimal: &str) -> String {
    let sep = separators(locale);
    let (head, tail) = decimal.split_once('.').unwrap_or((decimal, ""));
    let (sign, digits) = if let Some(rest) = head.strip_prefix('-') { ("-", rest) } else { ("", head) };
    let mut out = String::with_capacity(decimal.len() + 8);
    out.push_str(sign);
    let chars: Vec<char> = digits.chars().collect();
    for (i, c) in chars.iter().enumerate() {
        // South Asian 3-2-2 digit grouping.
        let remaining = chars.len() - i;
        if i != 0 && (if matches!(locale.language(), "hi" | "bn") { remaining > 3 && (remaining - 3) % 2 == 0 || remaining == 3 } else { remaining % 3 == 0 }) {
            out.push_str(sep.grouping);
        }
        out.push(*c);
    }
    if !tail.is_empty() { out.push_str(sep.decimal); out.push_str(tail); }
    out
}

/// Format a currency amount using an ISO 4217 code, with a narrow built-in
/// symbol table. Unknown currency codes display as ISO codes, not guessed signs.
pub fn format_currency(locale: &Locale, amount: f64, currency_code: &str) -> String {
    let code = currency_code.to_ascii_uppercase();
    let symbol = match code.as_str() {
        "USD" => "$", "CAD" => "CA$", "EUR" => "€", "GBP" => "£",
        "JPY" => "¥", "CNY" => "CN¥", "INR" => "₹", "KRW" => "₩",
        "BRL" => "R$", "RUB" => "₽", "AUD" => "A$", "CHF" => "CHF",
        _ => code.as_str(),
    };
    let decimals = if matches!(code.as_str(), "JPY" | "KRW") { 0 } else { 2 };
    let number = format_number(locale, amount, decimals);
    if matches!(locale.language(), "fr" | "de" | "es" | "it" | "pt" | "ru" | "pl" | "nl" | "sv" | "fi" | "cs") {
        format!("{number}\u{a0}{symbol}")
    } else { format!("{symbol}{number}") }
}

/// Gregorian date, validated without external date/time crates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Date { year: i32, month: u8, day: u8 }
impl Date {
    pub fn new(year: i32, month: u8, day: u8) -> Option<Self> {
        if !(1..=12).contains(&month) { return None; }
        let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
        let days = match month { 2 => if leap { 29 } else { 28 }, 4 | 6 | 9 | 11 => 30, _ => 31 };
        if day == 0 || day > days { None } else { Some(Self { year, month, day }) }
    }
    pub fn year(&self) -> i32 { self.year }
    pub fn month(&self) -> u8 { self.month }
    pub fn day(&self) -> u8 { self.day }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateStyle { Short, Long }

/// A small fixed set of date conventions. Long English/French month names are
/// translated; other languages use numeric months to avoid incorrect names.
pub fn format_date(locale: &Locale, date: Date, style: DateStyle) -> String {
    let lang = locale.language();
    let us = locale.as_str().eq_ignore_ascii_case("en-US");
    let east_asia = matches!(lang, "ja" | "zh" | "ko");
    if style == DateStyle::Long {
        const EN: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
        const FR: [&str; 12] = ["janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre", "novembre", "décembre"];
        let idx = date.month as usize - 1;
        if lang == "fr" { return format!("{} {} {}", date.day, FR[idx], date.year); }
        if lang == "en" {
            return if us { format!("{} {}, {}", EN[idx], date.day, date.year) }
                else { format!("{} {} {}", date.day, EN[idx], date.year) };
        }
    }
    if east_asia || matches!(lang, "sv" | "lt") {
        format!("{:04}-{:02}-{:02}", date.year, date.month, date.day)
    } else if us {
        format!("{:02}/{:02}/{:04}", date.month, date.day, date.year)
    } else {
        format!("{:02}/{:02}/{:04}", date.day, date.month, date.year)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn formatting_examples() {
        let fr = Locale::parse("fr-FR").unwrap();
        assert_eq!(format_number(&fr, 12345.5, 2), "12\u{202f}345,50");
        assert_eq!(format_currency(&fr, 9.0, "EUR"), "9,00\u{a0}€");
        assert_eq!(format_integer(&Locale::parse("hi-IN").unwrap(), 123456789), "12,34,56,789");
        let date = Date::new(2026, 10, 20).unwrap();
        assert_eq!(format_date(&Locale::parse("en-US").unwrap(), date, DateStyle::Long), "October 20, 2026");
        assert_eq!(format_date(&fr, date, DateStyle::Long), "20 octobre 2026");
        assert!(Date::new(2025, 2, 29).is_none());
    }
}
