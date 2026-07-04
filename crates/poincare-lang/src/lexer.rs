//! Hand-written lexer.
//!
//! Produces a token stream plus any lexical diagnostics. Newlines are emitted
//! as significant `Newline` tokens except when suppressed inside `()`/`[]`
//! groups or after a line-continuation token; consecutive/leading newlines are
//! collapsed. This makes the language newline-terminated but not
//! indentation-sensitive.

use crate::diagnostic::Diagnostic;
use crate::span::Span;
use crate::token::{Token, TokenKind};

/// Tokenize `src`, returning the tokens (always ending in `Eof`) and any
/// lexical diagnostics.
pub fn lex(src: &str) -> (Vec<Token>, Vec<Diagnostic>) {
    Lexer::new(src).run()
}

struct Lexer {
    chars: Vec<char>,
    /// Byte offset of each char; `offs[chars.len()]` is the source length.
    offs: Vec<u32>,
    pos: usize,
    tokens: Vec<Token>,
    diags: Vec<Diagnostic>,
    group_depth: i32,
}

impl Lexer {
    fn new(src: &str) -> Self {
        let mut chars = Vec::new();
        let mut offs = Vec::new();
        for (byte, ch) in src.char_indices() {
            chars.push(ch);
            offs.push(byte as u32);
        }
        offs.push(src.len() as u32);
        Self {
            chars,
            offs,
            pos: 0,
            tokens: Vec::new(),
            diags: Vec::new(),
            group_depth: 0,
        }
    }

    fn run(mut self) -> (Vec<Token>, Vec<Diagnostic>) {
        loop {
            self.skip_inline_whitespace();
            if self.at_end() {
                let off = self.offset();
                self.tokens.push(Token {
                    kind: TokenKind::Eof,
                    span: Span::new(off, off),
                });
                break;
            }
            let c = self.peek();
            if c == '\n' {
                self.handle_newline();
                self.pos += 1;
                continue;
            }
            if c == '#' {
                while !self.at_end() && self.peek() != '\n' {
                    self.pos += 1;
                }
                continue;
            }
            self.scan_token();
        }
        (self.tokens, self.diags)
    }

    // --- character helpers ---

    fn at_end(&self) -> bool {
        self.pos >= self.chars.len()
    }

    fn peek(&self) -> char {
        self.chars[self.pos]
    }

    fn peek_at(&self, k: usize) -> Option<char> {
        self.chars.get(self.pos + k).copied()
    }

    fn offset(&self) -> u32 {
        self.offs[self.pos]
    }

    fn skip_inline_whitespace(&mut self) {
        while !self.at_end() {
            let c = self.peek();
            if c == ' ' || c == '\t' || c == '\r' {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    // --- newline handling ---

    fn handle_newline(&mut self) {
        let suppress = self.group_depth > 0
            || matches!(self.tokens.last(), Some(t) if t.kind.is_continuation())
            || matches!(
                self.tokens.last().map(|t| &t.kind),
                None | Some(TokenKind::Newline)
            );
        if !suppress {
            let off = self.offset();
            self.tokens.push(Token {
                kind: TokenKind::Newline,
                span: Span::new(off, off + 1),
            });
        }
    }

    // --- token scanning ---

    fn push(&mut self, kind: TokenKind, start: usize) {
        let span = Span::new(self.offs[start], self.offset());
        match kind {
            TokenKind::LParen | TokenKind::LBracket => self.group_depth += 1,
            TokenKind::RParen | TokenKind::RBracket => {
                self.group_depth = (self.group_depth - 1).max(0)
            }
            _ => {}
        }
        self.tokens.push(Token { kind, span });
    }

    fn scan_token(&mut self) {
        let start = self.pos;
        let c = self.peek();

        if c.is_ascii_digit() {
            self.scan_number(start);
            return;
        }
        if c == '_' || c.is_alphabetic() {
            self.scan_ident(start);
            return;
        }
        if c == '"' {
            self.scan_string(start);
            return;
        }

        // Operators and punctuation.
        let two = self.peek_at(1);
        let kind = match c {
            '+' => Some(TokenKind::Plus),
            '-' if two == Some('>') => {
                self.pos += 1;
                Some(TokenKind::Arrow)
            }
            '-' => Some(TokenKind::Minus),
            '*' => Some(TokenKind::Star),
            '/' => Some(TokenKind::Slash),
            '%' => Some(TokenKind::Percent),
            '^' => Some(TokenKind::Caret),
            '=' if two == Some('=') => {
                self.pos += 1;
                Some(TokenKind::EqEq)
            }
            '=' if two == Some('>') => {
                self.pos += 1;
                Some(TokenKind::FatArrow)
            }
            '=' => Some(TokenKind::Eq),
            '!' if two == Some('=') => {
                self.pos += 1;
                Some(TokenKind::NotEq)
            }
            '<' if two == Some('=') => {
                self.pos += 1;
                Some(TokenKind::Le)
            }
            '<' => Some(TokenKind::Lt),
            '>' if two == Some('=') => {
                self.pos += 1;
                Some(TokenKind::Ge)
            }
            '>' => Some(TokenKind::Gt),
            '.' if two == Some('.') => {
                self.pos += 1;
                Some(TokenKind::DotDot)
            }
            '.' => Some(TokenKind::Dot),
            '∘' => Some(TokenKind::Dot),
            '|' if two == Some('>') => {
                self.pos += 1;
                Some(TokenKind::Pipe)
            }
            ':' => Some(TokenKind::Colon),
            ',' => Some(TokenKind::Comma),
            ';' => Some(TokenKind::Semicolon),
            '(' => Some(TokenKind::LParen),
            ')' => Some(TokenKind::RParen),
            '[' => Some(TokenKind::LBracket),
            ']' => Some(TokenKind::RBracket),
            '{' => Some(TokenKind::LBrace),
            '}' => Some(TokenKind::RBrace),
            _ => None,
        };

        match kind {
            Some(kind) => {
                self.pos += 1;
                self.push(kind, start);
            }
            None => {
                self.pos += 1;
                let span = Span::new(self.offs[start], self.offset());
                self.diags.push(Diagnostic::error(
                    format!("unexpected character `{c}`"),
                    span,
                ));
            }
        }
    }

    fn scan_number(&mut self, start: usize) {
        // Integer part.
        self.consume_digits();
        let mut is_float = false;

        // Fractional part: `.` followed by a digit (not `..` range).
        if self.peek_or_nul() == '.' && self.peek_at(1).map(|c| c.is_ascii_digit()).unwrap_or(false)
        {
            is_float = true;
            self.pos += 1; // '.'
            self.consume_digits();
        }

        // Exponent.
        if matches!(self.peek_or_nul(), 'e' | 'E') {
            let after = self.peek_at(1);
            let expo_ok = matches!(after, Some(c) if c.is_ascii_digit())
                || (matches!(after, Some('+') | Some('-'))
                    && matches!(self.peek_at(2), Some(c) if c.is_ascii_digit()));
            if expo_ok {
                is_float = true;
                self.pos += 1; // 'e'
                if matches!(self.peek_or_nul(), '+' | '-') {
                    self.pos += 1;
                }
                self.consume_digits();
            }
        }

        let text: String = self.chars[start..self.pos].iter().collect();
        let kind = if is_float {
            TokenKind::Float(text)
        } else {
            TokenKind::Int(text)
        };
        self.push(kind, start);
    }

    fn consume_digits(&mut self) {
        while !self.at_end() {
            let c = self.peek();
            if c.is_ascii_digit() || c == '_' {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn peek_or_nul(&self) -> char {
        if self.at_end() { '\0' } else { self.peek() }
    }

    fn scan_ident(&mut self, start: usize) {
        while !self.at_end() {
            let c = self.peek();
            if c == '_' || c.is_alphanumeric() {
                self.pos += 1;
            } else {
                break;
            }
        }
        let text: String = self.chars[start..self.pos].iter().collect();
        let kind = keyword_or_ident(text);
        self.push(kind, start);
    }

    fn scan_string(&mut self, start: usize) {
        self.pos += 1; // opening quote
        let mut value = String::new();
        loop {
            if self.at_end() {
                let span = Span::new(self.offs[start], self.offset());
                self.diags
                    .push(Diagnostic::error("unterminated string literal", span));
                break;
            }
            let c = self.peek();
            if c == '"' {
                self.pos += 1;
                break;
            }
            if c == '\\' {
                self.pos += 1;
                if self.at_end() {
                    let span = Span::new(self.offs[start], self.offset());
                    self.diags
                        .push(Diagnostic::error("unterminated string literal", span));
                    break;
                }
                let esc = self.peek();
                match esc {
                    '"' => value.push('"'),
                    '\\' => value.push('\\'),
                    'n' => value.push('\n'),
                    't' => value.push('\t'),
                    'r' => value.push('\r'),
                    other => {
                        let span = Span::new(self.offset(), self.offs[self.pos + 1]);
                        self.diags.push(Diagnostic::error(
                            format!("unknown escape `\\{other}`"),
                            span,
                        ));
                        value.push(other);
                    }
                }
                self.pos += 1;
                continue;
            }
            value.push(c);
            self.pos += 1;
        }
        self.push(TokenKind::Str(value), start);
    }
}

fn keyword_or_ident(text: String) -> TokenKind {
    match text.as_str() {
        "fn" => TokenKind::Fn,
        "for" => TokenKind::For,
        "in" => TokenKind::In,
        "if" => TokenKind::If,
        "else" => TokenKind::Else,
        "and" => TokenKind::And,
        "or" => TokenKind::Or,
        "not" => TokenKind::Not,
        "plot" => TokenKind::Plot,
        "over" => TokenKind::Over,
        "true" => TokenKind::True,
        "false" => TokenKind::False,
        // `data` is intentionally NOT reserved: it is too common a variable
        // name (the frozen examples use it), so a future ADT keyword must be
        // spelled differently.
        "Type" => TokenKind::Reserved("Type"),
        "match" => TokenKind::Reserved("match"),
        "forall" => TokenKind::Reserved("forall"),
        "let" => TokenKind::Reserved("let"),
        "fun" => TokenKind::Reserved("fun"),
        _ => TokenKind::Ident(text),
    }
}
