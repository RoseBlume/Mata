use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Kind {
    Ident(String),
    Number(f64),
    Dimension(f64, String),
    String(String),
    Symbol(&'static str),
    End,
}

#[derive(Debug, Clone)]
pub(crate) struct Token {
    pub kind: Kind,
    pub line: usize,
    pub column: usize,
}

pub(crate) fn lex(src: &str) -> Result<Vec<Token>> {
    const MAX_SOURCE: usize = 2 * 1024 * 1024;
    if src.len() > MAX_SOURCE { return Err(Error::runtime("source is larger than 2 MiB")); }
    let mut s = Lexer { input: src.chars().collect(), i: 0, line: 1, col: 1, out: Vec::new() };
    while let Some(c) = s.peek() {
        let (line, column) = (s.line, s.col);
        match c {
            ' ' | '\t' | '\r' => { s.advance(); }
            '\n' | ';' => { s.advance(); s.push(Kind::Symbol(";"), line, column); }
            '/' if s.next() == Some('/') => {
                s.advance(); s.advance();
                while let Some(x) = s.peek() { if x == '\n' { break; } s.advance(); }
            }
            '/' if s.next() == Some('*') => {
                s.advance(); s.advance();
                let mut closed = false;
                while let Some(x) = s.peek() {
                    if x == '*' && s.next() == Some('/') {
                        s.advance(); s.advance(); closed = true; break;
                    }
                    s.advance();
                }
                if !closed { return Err(Error::new("unterminated comment", line, column)); }
            }
            '"' | '\'' => {
                let quote = c; s.advance();
                let mut text = String::new();
                let mut closed = false;
                while let Some(ch) = s.peek() {
                    if ch == quote { s.advance(); closed = true; break; }
                    if ch == '\n' { return Err(Error::new("newline in quoted string", s.line, s.col)); }
                    if ch == '\\' {
                        s.advance();
                        let escaped = s.peek().ok_or_else(|| Error::new("unterminated escape", s.line, s.col))?;
                        s.advance();
                        text.push(match escaped {
                            'n' => '\n', 't' => '\t', 'r' => '\r', '0' => '\0',
                            '\\' => '\\', '"' => '"', '\'' => '\'',
                            _ => return Err(Error::new("invalid escape sequence", s.line, s.col)),
                        });
                    } else { text.push(ch); s.advance(); }
                }
                if !closed { return Err(Error::new("unterminated string", line, column)); }
                s.push(Kind::String(text), line, column);
            }
            '#' => {
                s.advance();
                let mut hex = String::from("#");
                while let Some(x) = s.peek() {
                    if x.is_ascii_hexdigit() { hex.push(x); s.advance(); } else { break; }
                }
                if !matches!(hex.len(), 4 | 5 | 7 | 9) {
                    return Err(Error::new("expected #RGB, #RGBA, #RRGGBB, or #RRGGBBAA", line, column));
                }
                s.push(Kind::String(hex), line, column);
            }
            x if x.is_ascii_digit() => {
                let mut n = String::new();
                while let Some(x) = s.peek() {
                    if x.is_ascii_digit() { n.push(x); s.advance(); } else { break; }
                }
                if s.peek() == Some('.') && s.next().is_some_and(|x| x.is_ascii_digit()) {
                    n.push('.'); s.advance();
                    while let Some(x) = s.peek() {
                        if x.is_ascii_digit() { n.push(x); s.advance(); } else { break; }
                    }
                }
                // 'e' is also the beginning of valid CSS units like em/ex.
                // Consume it as a scientific exponent only if the lookahead
                // contains exponent digits (optionally preceded by a sign).
                if matches!(s.peek(), Some('e' | 'E')) {
                    let after_e = s.next();
                    let has_sign = matches!(after_e, Some('+' | '-'));
                    let valid_exponent = after_e.is_some_and(|x| x.is_ascii_digit())
                        || (has_sign && s.input.get(s.i + 2).is_some_and(|x| x.is_ascii_digit()));
                    if valid_exponent {
                        n.push(s.advance().unwrap());
                        if has_sign { n.push(s.advance().unwrap()); }
                        while s.peek().is_some_and(|x| x.is_ascii_digit()) {
                            n.push(s.advance().unwrap());
                        }
                    } else if has_sign || !after_e.is_some_and(|x| x.is_ascii_alphabetic()) {
                        return Err(Error::new("malformed exponent", line, column));
                    }
                }
                let value = n.parse::<f64>().map_err(|_| Error::new("invalid number", line, column))?;
                if !value.is_finite() { return Err(Error::new("number is not finite", line, column)); }
                // Unit suffixes are part of the number token. A `%` adjacent to
                // a digit, as in `10%`, is a percentage; spaced `%` is modulo.
                let mut unit = String::new();
                while let Some(x) = s.peek() {
                    if x.is_ascii_alphabetic() { unit.push(x); s.advance(); } else { break; }
                }
                if unit.is_empty() && s.peek() == Some('%') &&
                    !s.next().is_some_and(|x| x.is_ascii_digit()) {
                    unit.push('%'); s.advance();
                }
                if unit.is_empty() { s.push(Kind::Number(value), line, column); }
                else if matches!(unit.as_str(), "px" | "%" | "vw" | "vh" | "vmin" | "vmax" | "em" | "rem" | "ch" | "ex" | "pt") {
                    s.push(Kind::Dimension(value, unit), line, column);
                } else { return Err(Error::new(format!("unsupported length unit '{unit}'"), line, column)); }
            }
            x if x == '_' || x.is_alphabetic() => {
                let mut word = String::new();
                while let Some(x) = s.peek() {
                    if x == '_' || x.is_alphanumeric() { word.push(x); s.advance(); }
                    else { break; }
                }
                s.push(Kind::Ident(word), line, column);
            }
            _ => {
                let two = match (c, s.next()) {
                    ('=', Some('=')) => Some("=="), ('!', Some('=')) => Some("!="),
                    ('<', Some('=')) => Some("<="), ('>', Some('=')) => Some(">="),
                    ('&', Some('&')) => Some("&&"), ('|', Some('|')) => Some("||"),
                    ('+', Some('=')) => Some("+="), ('-', Some('=')) => Some("-="),
                    ('*', Some('=')) => Some("*="), ('/', Some('=')) => Some("/="),
                    _ => None,
                };
                if let Some(op) = two { s.advance(); s.advance(); s.push(Kind::Symbol(op), line, column); }
                else {
                    let token = match c {
                        '{' => "{", '}' => "}", '(' => "(", ')' => ")", '[' => "[", ']' => "]", ':' => ":",
                        '.' => ".", ',' => ",", '?' => "?", '=' => "=", '+' => "+",
                        '-' => "-", '*' => "*", '/' => "/", '%' => "%", '!' => "!",
                        '<' => "<", '>' => ">", _ => return Err(Error::new(format!("unexpected character '{c}'"), line, column)),
                    };
                    s.advance(); s.push(Kind::Symbol(token), line, column);
                }
            }
        }
        if s.out.len() > 100_000 { return Err(Error::new("too many tokens", line, column)); }
    }
    s.push(Kind::End, s.line, s.col);
    Ok(s.out)
}

struct Lexer {
    input: Vec<char>, i: usize, line: usize, col: usize, out: Vec<Token>,
}
impl Lexer {
    fn peek(&self) -> Option<char> { self.input.get(self.i).copied() }
    fn next(&self) -> Option<char> { self.input.get(self.i + 1).copied() }
    fn advance(&mut self) -> Option<char> {
        let c = self.peek()?; self.i += 1;
        if c == '\n' { self.line += 1; self.col = 1; }
        else { self.col += 1; }
        Some(c)
    }
    fn push(&mut self, kind: Kind, line: usize, column: usize) { self.out.push(Token {kind, line, column}); }
}
