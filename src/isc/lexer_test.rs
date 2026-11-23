use super::{Lexed, Token, tokenize};
use crate::isc::ErrorKind;

fn word(text: &str) -> Token {
    Token::Word(text.to_owned())
}

fn tokens(text: &str) -> Vec<Token> {
    tokenize(text)
        .expect("tokenizes")
        .into_iter()
        .map(|lexed| lexed.token)
        .collect()
}

#[test]
fn splits_words_strings_and_punctuation_with_their_lines() {
    let lexed = tokenize("lease 10.0.0.1 {\n  uid \"a b\";\n}").expect("tokenizes");
    let expected = [
        (word("lease"), 1),
        (word("10.0.0.1"), 1),
        (Token::Open, 1),
        (word("uid"), 2),
        (Token::Str("a b".into()), 2),
        (Token::Semi, 2),
        (Token::Close, 3),
    ];
    let expected: Vec<Lexed> = expected
        .into_iter()
        .map(|(token, line)| Lexed { token, line })
        .collect();
    assert_eq!(lexed, expected);
}

#[test]
fn comments_are_skipped_outside_strings_only() {
    assert_eq!(
        tokens("a#b; c\n\"#d\" # e\nf"),
        [word("a"), Token::Str("#d".into()), word("f")]
    );
}

#[test]
fn string_escapes_are_decoded() {
    assert_eq!(
        tokens(r#""\"\\\t\r\n\b\q\101\0\12""#),
        [Token::Str("\"\\\t\r\n\u{8}qA\0\n".into())]
    );
}

#[test]
fn octal_escapes_that_are_not_utf8_become_replacement_characters() {
    assert_eq!(tokens(r#""a\377b""#), [Token::Str("a\u{fffd}b".into())]);
}

#[test]
fn strings_span_lines_and_keep_counting_them() {
    let lexed = tokenize("\"a\nb\" c").expect("tokenizes");
    assert_eq!(lexed[0].line, 1);
    assert_eq!(
        lexed[1],
        Lexed {
            token: word("c"),
            line: 2
        }
    );
}

#[test]
fn octal_escape_above_255_is_an_error() {
    let err = tokenize("\n\"\\400\"").expect_err("out of range");
    assert_eq!((err.line(), err.kind()), (2, &ErrorKind::Escape(0o400)));
}

#[test]
fn unterminated_string_is_an_error_on_its_opening_line() {
    for text in ["x\n\"abc\n;", "x\n\"abc\\"] {
        let err = tokenize(text).expect_err("unterminated");
        assert_eq!(
            (err.line(), err.kind()),
            (2, &ErrorKind::UnterminatedString),
            "{text:?}"
        );
    }
}
