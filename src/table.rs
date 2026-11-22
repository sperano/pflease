//! Plain-text table of leases for `pflease list`.
//!
//! Column widths come from the content, so no table crate is needed: the
//! output is left-aligned text with a fixed gap, no borders and no colour,
//! which reads the same on a terminal, in a pipe and in a file.

use std::fmt::Write as _;

use jiff::Timestamp;
use jiff::tz::TimeZone;

use crate::{Lease, LeaseSource, SourceError};

/// Column headings, in display order.
const HEADERS: [&str; COLUMNS] = [
    "IP",
    "MAC",
    "HOSTNAME",
    "START",
    "END",
    "STATE",
    "INTERFACE",
];
/// Number of columns in the table.
const COLUMNS: usize = 7;
/// Spaces between two columns.
const GAP: &str = "  ";
/// What a cell shows when the lease has no value for it.
const MISSING: &str = "-";
/// Fixed `strftime` format of the start and end times.
const TIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

/// Fetches every lease from `source` and renders them as a table, ordered
/// by IP address, with times shown in `tz`.
///
/// # Errors
///
/// Returns the source's [`SourceError`] when it cannot produce its leases.
pub fn list(source: &dyn LeaseSource, tz: &TimeZone) -> Result<String, SourceError> {
    let mut leases = source.leases()?;
    leases.sort_by_key(|lease| lease.ip);
    Ok(render(&leases, tz))
}

/// Renders `leases` in the given order as a header line followed by one
/// line per lease, each ending in a newline.
#[must_use]
pub fn render(leases: &[Lease], tz: &TimeZone) -> String {
    let rows: Vec<[String; COLUMNS]> = leases.iter().map(|lease| cells(lease, tz)).collect();
    let widths = column_widths(&rows);
    let mut out = String::new();
    push_row(&mut out, &HEADERS, &widths);
    for row in &rows {
        push_row(&mut out, row, &widths);
    }
    out
}

/// The displayed text of each of the lease's columns.
fn cells(lease: &Lease, tz: &TimeZone) -> [String; COLUMNS] {
    [
        lease.ip.to_string(),
        lease.mac.to_string(),
        optional_text(lease.hostname.as_deref()),
        optional_time(lease.start, tz),
        optional_time(lease.end, tz),
        sanitize(lease.state.as_str()),
        optional_text(lease.interface.as_deref()),
    ]
}

fn optional_text(value: Option<&str>) -> String {
    value.map_or_else(|| MISSING.to_owned(), sanitize)
}

fn optional_time(value: Option<Timestamp>, tz: &TimeZone) -> String {
    value.map_or_else(
        || MISSING.to_owned(),
        |time| time.to_zoned(tz.clone()).strftime(TIME_FORMAT).to_string(),
    )
}

/// Escapes control characters so text a DHCP client chose (a hostname)
/// cannot move the cursor or recolour the terminal, and so every character
/// left takes one column; an empty value shows as missing.
fn sanitize(text: &str) -> String {
    if text.is_empty() {
        return MISSING.to_owned();
    }
    text.chars().fold(String::new(), |mut out, c| {
        if c.is_control() {
            let _ = write!(out, "{}", c.escape_unicode());
        } else {
            out.push(c);
        }
        out
    })
}

/// The widest cell of each column, headers included, in characters.
fn column_widths(rows: &[[String; COLUMNS]]) -> [usize; COLUMNS] {
    let mut widths = HEADERS.map(str::len);
    for row in rows {
        for (width, cell) in widths.iter_mut().zip(row) {
            *width = (*width).max(cell.chars().count());
        }
    }
    widths
}

/// Appends one line: every cell but the last padded to its column's width,
/// so lines carry no trailing spaces.
fn push_row<S: AsRef<str>>(out: &mut String, row: &[S; COLUMNS], widths: &[usize; COLUMNS]) {
    let last = COLUMNS - 1;
    for (index, (cell, width)) in row.iter().zip(widths).enumerate() {
        let cell = cell.as_ref();
        if index == last {
            out.push_str(cell);
        } else {
            let _ = write!(out, "{cell:<width$}{GAP}");
        }
    }
    out.push('\n');
}

#[cfg(test)]
#[path = "table_test.rs"]
mod tests;
