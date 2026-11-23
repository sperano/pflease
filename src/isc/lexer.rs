//! Tokens of the `dhcpd.leases` grammar: bare words, quoted strings, `;`,
//! `{` and `}`, each with its 1-based line. `#` starts a comment to the end
//! of the line outside a string.

use super::{ErrorKind, ParseLeasesError};

/// Base of the `\ooo` escapes ISC writes for unprintable string bytes.
const OCTAL_RADIX: u32 = 8;
/// Most digits in one octal escape.
const MAX_OCTAL_DIGITS: usize = 3;

/// One token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Token {
    /// A bare word: a keyword, an address, a date, a number.
    Word(String),
    /// A quoted string, escapes decoded; bytes that are not UTF-8 become
    /// U+FFFD.
    Str(String),
    /// `;`, the end of a statement.
    Semi,
    /// `{`, the start of a block.
    Open,
    /// `}`, the end of a block.
    Close,
}

/// A token and the line it starts on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Lexed {
    pub(super) token: Token,
    pub(super) line: usize,
}

/// Splits `text` into tokens.
pub(super) fn tokenize(text: &str) -> Result<Vec<Lexed>, ParseLeasesError> {
    let mut lexer = Lexer {
        chars: text.chars().peekable(),
        line: 1,
    };
    let mut tokens = Vec::new();
    while let Some(lexed) = lexer.next_token()? {
        tokens.push(lexed);
    }
    Ok(tokens)
}

struct Lexer<'a> {
    chars: std::iter::Peekable<std::str::Chars<'a>>,
    line: usize,
}

impl Lexer<'_> {
    fn next_token(&mut self) -> Result<Option<Lexed>, ParseLeasesError> {
        self.skip_blanks();
        let line = self.line;
        let Some(c) = self.chars.next() else {
            return Ok(None);
        };
        let token = match c {
            ';' => Token::Semi,
            '{' => Token::Open,
            '}' => Token::Close,
            '"' => Token::Str(self.string(line)?),
            c => Token::Word(self.word(c)),
        };
        Ok(Some(Lexed { token, line }))
    }

    /// Skips whitespace and comments, counting lines.
    fn skip_blanks(&mut self) {
        while let Some(&c) = self.chars.peek() {
            if c == '#' {
                while self.chars.next_if(|&c| c != '\n').is_some() {}
            } else if c.is_whitespace() {
                self.bump();
            } else {
                return;
            }
        }
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.chars.next()?;
        if c == '\n' {
            self.line += 1;
        }
        Some(c)
    }

    fn word(&mut self, first: char) -> String {
        let mut word = String::from(first);
        while let Some(c) = self.chars.next_if(|&c| !ends_word(c)) {
            word.push(c);
        }
        word
    }

    /// The rest of a string whose opening quote, on `line`, is consumed.
    fn string(&mut self, line: usize) -> Result<String, ParseLeasesError> {
        let mut bytes = Vec::new();
        loop {
            match self.bump() {
                None => return Err(ParseLeasesError::new(line, ErrorKind::UnterminatedString)),
                Some('"') => return Ok(String::from_utf8_lossy(&bytes).into_owned()),
                Some('\\') => self.escape(line, &mut bytes)?,
                Some(c) => bytes.extend_from_slice(c.encode_utf8(&mut [0; 4]).as_bytes()),
            }
        }
    }

    /// Decodes the escape after a `\`: `\ooo` octal, `\t`, `\r`, `\n`, `\b`,
    /// and any other character as itself (`\"`, `\\`).
    fn escape(&mut self, line: usize, bytes: &mut Vec<u8>) -> Result<(), ParseLeasesError> {
        let Some(c) = self.bump() else {
            return Err(ParseLeasesError::new(line, ErrorKind::UnterminatedString));
        };
        if let Some(digit) = c.to_digit(OCTAL_RADIX) {
            bytes.push(self.octal(digit, line)?);
            return Ok(());
        }
        let decoded = match c {
            't' => '\t',
            'r' => '\r',
            'n' => '\n',
            'b' => '\u{8}',
            other => other,
        };
        bytes.extend_from_slice(decoded.encode_utf8(&mut [0; 4]).as_bytes());
        Ok(())
    }

    /// An octal escape whose first digit is `first`.
    fn octal(&mut self, first: u32, line: usize) -> Result<u8, ParseLeasesError> {
        let mut value = first;
        for _ in 1..MAX_OCTAL_DIGITS {
            match self.chars.next_if(|c| c.is_digit(OCTAL_RADIX)) {
                Some(c) => value = value * OCTAL_RADIX + c.to_digit(OCTAL_RADIX).unwrap_or(0),
                None => break,
            }
        }
        u8::try_from(value).map_err(|_| ParseLeasesError::new(line, ErrorKind::Escape(value)))
    }
}

/// Whether `c` ends a bare word.
fn ends_word(c: char) -> bool {
    c.is_whitespace() || matches!(c, ';' | '{' | '}' | '"' | '#')
}

#[cfg(test)]
#[path = "lexer_test.rs"]
mod tests;
