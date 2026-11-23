//! Parser for ISC `dhcpd`'s lease database, `dhcpd.leases(5)`.
//!
//! The file is a series of top-level statements; only `lease <ip> { ... }`
//! blocks matter here, and every other statement or block
//! (`authoring-byte-order`, `server-duid`, `failover peer`, `host`) is
//! skipped. ISC appends a new block whenever a lease changes, so the last
//! block for an address is the current one, whatever its times say.
//!
//! Inside a lease, `starts`, `ends`, `binding state`, `hardware` and
//! `client-hostname` are read, and other statements (`tstp`, `cltt`, `uid`,
//! `next binding state`, `set`, `on ... { }`) are ignored. Decisions per
//! case:
//!
//! - A block with no `hardware` statement is dropped: it records an address
//!   (an abandoned or never-used one), not a client, and a [`Lease`] always
//!   has a MAC. It still supersedes earlier blocks for its address.
//! - A missing `binding state`, a value that does not parse, a `hardware`
//!   type other than `ethernet`, an unbalanced brace, an unterminated string
//!   or statement is an error naming its line: the file is not what pflease
//!   understands, and guessing would show wrong leases.
//! - An unknown binding state is kept as [`LeaseState::Other`].
//! - Times are UTC (`W YYYY/MM/DD HH:MM:SS`, ISC's default `db-time-format`)
//!   or `epoch <seconds>` (the `local` format, whose trailing comment is
//!   ignored); `never` is no time. An empty `client-hostname` is none.
//! - The interface comes from [`Subnets`]; an address outside every subnet
//!   has none.

mod lexer;

use std::collections::HashMap;
use std::net::Ipv4Addr;

use jiff::Timestamp;
use jiff::civil::DateTime;
use jiff::tz::TimeZone;

use crate::{Lease, LeaseState, MacAddr, Subnets};
use lexer::{Lexed, Token};

/// Keyword that opens a lease block.
const LEASE: &str = "lease";
/// The only hardware type a [`MacAddr`] can hold.
const ETHERNET: &str = "ethernet";
/// Format of a time in ISC's default `db-time-format`, after the weekday.
const TIME_FORMAT: &str = "%Y/%m/%d %H:%M:%S";
/// Keyword of a time in seconds since the Unix epoch.
const EPOCH: &str = "epoch";
/// Keyword of a time that never comes.
const NEVER: &str = "never";
/// Separator between MAC octets in ISC's output.
const MAC_SEPARATOR: char = ':';
/// Radix of a MAC octet.
const HEX_RADIX: u32 = 16;
/// Most hex digits ISC writes per octet; it may omit a leading zero.
const MAX_OCTET_DIGITS: usize = 2;

/// Why a lease database could not be parsed, with the 1-based line.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("line {line}: {kind}")]
pub struct ParseLeasesError {
    line: usize,
    kind: ErrorKind,
}

impl ParseLeasesError {
    pub(crate) const fn new(line: usize, kind: ErrorKind) -> Self {
        Self { line, kind }
    }

    /// The 1-based line the error is on.
    #[must_use]
    pub const fn line(&self) -> usize {
        self.line
    }

    /// What is wrong.
    #[must_use]
    pub const fn kind(&self) -> &ErrorKind {
        &self.kind
    }
}

/// What is wrong in a lease database.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ErrorKind {
    /// The file is not UTF-8.
    #[error("not UTF-8 text")]
    NotUtf8,
    /// A string has no closing quote.
    #[error("unterminated string")]
    UnterminatedString,
    /// An octal escape is above 255; holds its value.
    #[error("string escape \\{0:o} is above \\377")]
    Escape(u32),
    /// A block is never closed; the line is the block's.
    #[error("block is never closed")]
    UnclosedBlock,
    /// A `}` closes no block.
    #[error("`}}` closes no block")]
    UnexpectedClose,
    /// A statement does not end with `;`.
    #[error("statement does not end with `;`")]
    Unterminated,
    /// `lease` is not followed by exactly one word before its block.
    #[error("`lease` must be followed by one address and a block")]
    LeaseHeader,
    /// The lease address is not IPv4; holds it.
    #[error("{0:?} is not an IPv4 address")]
    Address(String),
    /// A statement's value does not parse.
    #[error("invalid `{field}` value {value:?}")]
    Value {
        /// The statement's keyword.
        field: &'static str,
        /// Its value, words joined by spaces.
        value: String,
    },
    /// A `hardware` type other than `ethernet`; holds it.
    #[error("hardware type {0:?} is not supported, only ethernet")]
    Hardware(String),
    /// A lease block has no `binding state`; the line is the block's.
    #[error("lease has no `binding state`")]
    MissingState,
}

/// [`parse`] for raw bytes, which must be UTF-8.
///
/// # Errors
///
/// Returns [`ErrorKind::NotUtf8`] on the line of the first invalid byte, or
/// what [`parse`] returns.
pub fn parse_bytes(bytes: &[u8], subnets: &Subnets) -> Result<Vec<Lease>, ParseLeasesError> {
    let text = std::str::from_utf8(bytes).map_err(|err| {
        let valid = &bytes[..err.valid_up_to()];
        let line = valid.split(|&b| b == b'\n').count();
        ParseLeasesError::new(line, ErrorKind::NotUtf8)
    })?;
    parse(text, subnets)
}

/// The current leases in `text`, a `dhcpd.leases` file, in the order their
/// addresses first appear, with interfaces from `subnets`.
///
/// # Errors
///
/// Returns a [`ParseLeasesError`] naming the line of the first statement
/// pflease cannot read; see the module documentation for each case.
pub fn parse(text: &str, subnets: &Subnets) -> Result<Vec<Lease>, ParseLeasesError> {
    let mut parser = Parser {
        tokens: lexer::tokenize(text)?.into_iter(),
    };
    let mut order: Vec<(Ipv4Addr, Option<Lease>)> = Vec::new();
    let mut index: HashMap<Ipv4Addr, usize> = HashMap::new();
    while let Some((ip, lease)) = parser.next_lease(subnets)? {
        if let Some(&at) = index.get(&ip) {
            order[at].1 = lease;
        } else {
            index.insert(ip, order.len());
            order.push((ip, lease));
        }
    }
    Ok(order.into_iter().filter_map(|(_, lease)| lease).collect())
}

/// Words up to a punctuation token, and that token.
struct Head {
    /// Line of the first word, or of the end when there is no word.
    line: usize,
    words: Vec<String>,
    end: End,
    /// Line of the end token; the last line read at the end of input.
    end_line: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum End {
    Semi,
    Open,
    Close,
    Eof,
}

struct Parser {
    tokens: std::vec::IntoIter<Lexed>,
}

impl Parser {
    /// The next lease block's address and lease (`None` without hardware);
    /// `None` at the end of input.
    fn next_lease(
        &mut self,
        subnets: &Subnets,
    ) -> Result<Option<(Ipv4Addr, Option<Lease>)>, ParseLeasesError> {
        loop {
            let head = self.head();
            match head.end {
                End::Eof if head.words.is_empty() => return Ok(None),
                End::Eof => return Err(ParseLeasesError::new(head.line, ErrorKind::Unterminated)),
                End::Close => {
                    return Err(ParseLeasesError::new(
                        head.end_line,
                        ErrorKind::UnexpectedClose,
                    ));
                }
                End::Semi => {}
                End::Open if head.words.first().is_some_and(|word| word == LEASE) => {
                    let body = self.lease_body(head.end_line)?;
                    return lease(&head, &body, subnets).map(Some);
                }
                End::Open => self.skip_block(head.end_line)?,
            }
        }
    }

    fn head(&mut self) -> Head {
        let mut words = Vec::new();
        let mut line = None;
        let mut last_line = 1;
        for Lexed { token, line: at } in self.tokens.by_ref() {
            last_line = at;
            let end = match token {
                Token::Word(word) | Token::Str(word) => {
                    line.get_or_insert(at);
                    words.push(word);
                    continue;
                }
                Token::Semi => End::Semi,
                Token::Open => End::Open,
                Token::Close => End::Close,
            };
            return Head {
                line: line.unwrap_or(at),
                words,
                end,
                end_line: at,
            };
        }
        Head {
            line: line.unwrap_or(last_line),
            words,
            end: End::Eof,
            end_line: last_line,
        }
    }

    /// The statements of a lease block opened on `open_line`, up to its `}`;
    /// nested blocks are skipped.
    fn lease_body(&mut self, open_line: usize) -> Result<Vec<Head>, ParseLeasesError> {
        let mut statements = Vec::new();
        loop {
            let head = self.head();
            match head.end {
                End::Semi => statements.push(head),
                End::Open => self.skip_block(head.end_line)?,
                End::Close if head.words.is_empty() => return Ok(statements),
                End::Close => {
                    return Err(ParseLeasesError::new(head.line, ErrorKind::Unterminated));
                }
                End::Eof => return Err(ParseLeasesError::new(open_line, ErrorKind::UnclosedBlock)),
            }
        }
    }

    /// Skips to the `}` closing the block opened on `open_line`.
    fn skip_block(&mut self, open_line: usize) -> Result<(), ParseLeasesError> {
        let mut depth = 1_usize;
        while depth > 0 {
            match self.tokens.next().map(|lexed| lexed.token) {
                Some(Token::Open) => depth += 1,
                Some(Token::Close) => depth -= 1,
                Some(_) => {}
                None => return Err(ParseLeasesError::new(open_line, ErrorKind::UnclosedBlock)),
            }
        }
        Ok(())
    }
}

/// The lease of a block whose header is `head` and statements `body`.
fn lease(
    head: &Head,
    body: &[Head],
    subnets: &Subnets,
) -> Result<(Ipv4Addr, Option<Lease>), ParseLeasesError> {
    let [_, address] = head.words.as_slice() else {
        return Err(ParseLeasesError::new(head.line, ErrorKind::LeaseHeader));
    };
    let ip: Ipv4Addr = address
        .parse()
        .map_err(|_| ParseLeasesError::new(head.line, ErrorKind::Address(address.clone())))?;
    let mut fields = Fields::default();
    for statement in body {
        fields
            .read(&statement.words)
            .map_err(|kind| ParseLeasesError::new(statement.line, kind))?;
    }
    let state = fields
        .state
        .ok_or_else(|| ParseLeasesError::new(head.line, ErrorKind::MissingState))?;
    let lease = fields.mac.map(|mac| Lease {
        ip,
        mac,
        hostname: fields.hostname,
        start: fields.start,
        end: fields.end,
        state,
        interface: subnets.interface_of(ip).map(str::to_owned),
    });
    Ok((ip, lease))
}

/// What a lease block's statements say; a repeated statement's last value
/// wins.
#[derive(Default)]
struct Fields {
    start: Option<Timestamp>,
    end: Option<Timestamp>,
    state: Option<LeaseState>,
    mac: Option<MacAddr>,
    hostname: Option<String>,
}

impl Fields {
    fn read(&mut self, words: &[String]) -> Result<(), ErrorKind> {
        let words: Vec<&str> = words.iter().map(String::as_str).collect();
        match words.as_slice() {
            ["starts", rest @ ..] => self.start = time("starts", rest)?,
            ["ends", rest @ ..] => self.end = time("ends", rest)?,
            ["binding", "state", rest @ ..] => self.state = Some(state(rest)?),
            ["hardware", rest @ ..] => self.mac = Some(hardware(rest)?),
            ["client-hostname", rest @ ..] => {
                let [name] = rest else {
                    return Err(invalid("client-hostname", rest));
                };
                self.hostname = Some((*name).to_owned()).filter(|name| !name.is_empty());
            }
            _ => {}
        }
        Ok(())
    }
}

fn invalid(field: &'static str, words: &[&str]) -> ErrorKind {
    ErrorKind::Value {
        field,
        value: words.join(" "),
    }
}

/// A `starts`/`ends` value: `W YYYY/MM/DD HH:MM:SS` in UTC, `epoch <s>`
/// or `never`.
fn time(field: &'static str, words: &[&str]) -> Result<Option<Timestamp>, ErrorKind> {
    let parsed = match words {
        [NEVER] => return Ok(None),
        [EPOCH, seconds] => seconds
            .parse()
            .ok()
            .and_then(|seconds| Timestamp::from_second(seconds).ok()),
        [weekday, date, clock]
            if weekday.len() == 1 && weekday.bytes().all(|b| b.is_ascii_digit()) =>
        {
            DateTime::strptime(TIME_FORMAT, format!("{date} {clock}"))
                .ok()
                .and_then(|civil| TimeZone::UTC.to_timestamp(civil).ok())
        }
        _ => None,
    };
    parsed.map(Some).ok_or_else(|| invalid(field, words))
}

fn state(words: &[&str]) -> Result<LeaseState, ErrorKind> {
    match words {
        [name] => name.parse().map_err(|_| invalid("binding state", words)),
        _ => Err(invalid("binding state", words)),
    }
}

/// A `hardware ethernet <mac>` value. ISC may write an octet with one
/// digit (`0:1:...`), which [`MacAddr::from_str`] refuses.
fn hardware(words: &[&str]) -> Result<MacAddr, ErrorKind> {
    match words {
        [ETHERNET, mac] => isc_mac(mac).ok_or_else(|| invalid("hardware ethernet", &[mac])),
        [kind, _] if *kind != ETHERNET => Err(ErrorKind::Hardware((*kind).to_owned())),
        _ => Err(invalid("hardware", words)),
    }
}

fn isc_mac(text: &str) -> Option<MacAddr> {
    let mut octets = [0_u8; crate::mac::OCTETS];
    let mut parts = text.split(MAC_SEPARATOR);
    for octet in &mut octets {
        let part = parts.next()?;
        let valid = (1..=MAX_OCTET_DIGITS).contains(&part.len())
            && part.bytes().all(|b| b.is_ascii_hexdigit());
        *octet = if valid {
            u8::from_str_radix(part, HEX_RADIX).ok()?
        } else {
            return None;
        };
    }
    parts.next().is_none().then_some(MacAddr::new(octets))
}

#[cfg(test)]
#[path = "isc_test.rs"]
mod tests;
