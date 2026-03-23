use crate::diag::{Diagnostic, Span};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TokenKind {
    Ident(String),
    Int(i64),
    Text(String),
    Module,
    Use,
    As,
    Effect,
    Cap,
    Type,
    Fn,
    Actor,
    On,
    Where,
    Let,
    Mut,
    Require,
    Return,
    If,
    Else,
    Match,
    True,
    False,
    LParen,
    RParen,
    LBrace,
    RBrace,
    Comma,
    Dot,
    Colon,
    Semicolon,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Bang,
    Pipe,
    AmpAmp,
    PipePipe,
    Equal,
    EqEq,
    NotEq,
    Less,
    LessEq,
    Greater,
    GreaterEq,
    Arrow,
    FatArrow,
    Eof,
}

#[derive(Clone, Debug)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

pub fn lex(source: &str) -> Result<Vec<Token>, Vec<Diagnostic>> {
    let mut lexer = Lexer::new(source);
    lexer.lex_all();
    if lexer.diagnostics.is_empty() {
        Ok(lexer.tokens)
    } else {
        Err(lexer.diagnostics)
    }
}

struct Lexer {
    chars: Vec<char>,
    cursor: usize,
    line: usize,
    column: usize,
    tokens: Vec<Token>,
    diagnostics: Vec<Diagnostic>,
}

impl Lexer {
    fn new(source: &str) -> Self {
        Self {
            chars: source.chars().collect(),
            cursor: 0,
            line: 1,
            column: 1,
            tokens: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    fn lex_all(&mut self) {
        while let Some(ch) = self.peek() {
            if ch.is_whitespace() {
                self.consume_whitespace();
                continue;
            }

            if ch == '/' && self.peek_next() == Some('/') {
                self.consume_comment();
                continue;
            }

            let line = self.line;
            let column = self.column;
            let offset = self.cursor;

            match ch {
                'a'..='z' | 'A'..='Z' | '_' => self.lex_ident_or_keyword(line, column, offset),
                '0'..='9' => self.lex_number(line, column, offset),
                '"' => self.lex_string(line, column, offset),
                '(' => self.push_simple(TokenKind::LParen, line, column, offset),
                ')' => self.push_simple(TokenKind::RParen, line, column, offset),
                '{' => self.push_simple(TokenKind::LBrace, line, column, offset),
                '}' => self.push_simple(TokenKind::RBrace, line, column, offset),
                ',' => self.push_simple(TokenKind::Comma, line, column, offset),
                '.' => self.push_simple(TokenKind::Dot, line, column, offset),
                ':' => self.push_simple(TokenKind::Colon, line, column, offset),
                ';' => self.push_simple(TokenKind::Semicolon, line, column, offset),
                '+' => self.push_simple(TokenKind::Plus, line, column, offset),
                '*' => self.push_simple(TokenKind::Star, line, column, offset),
                '%' => self.push_simple(TokenKind::Percent, line, column, offset),
                '-' => {
                    self.advance();
                    if self.peek() == Some('>') {
                        self.advance();
                        self.tokens.push(Token {
                            kind: TokenKind::Arrow,
                            span: Span::new(line, column, offset, 2),
                        });
                    } else {
                        self.tokens.push(Token {
                            kind: TokenKind::Minus,
                            span: Span::new(line, column, offset, 1),
                        });
                    }
                }
                '=' => {
                    self.advance();
                    let kind = if self.peek() == Some('=') {
                        self.advance();
                        TokenKind::EqEq
                    } else if self.peek() == Some('>') {
                        self.advance();
                        TokenKind::FatArrow
                    } else {
                        TokenKind::Equal
                    };
                    let len = self.cursor.saturating_sub(offset);
                    self.tokens.push(Token {
                        kind,
                        span: Span::new(line, column, offset, len),
                    });
                }
                '!' => {
                    self.advance();
                    let kind = if self.peek() == Some('=') {
                        self.advance();
                        TokenKind::NotEq
                    } else {
                        TokenKind::Bang
                    };
                    let len = self.cursor.saturating_sub(offset);
                    self.tokens.push(Token {
                        kind,
                        span: Span::new(line, column, offset, len),
                    });
                }
                '<' => {
                    self.advance();
                    let kind = if self.peek() == Some('=') {
                        self.advance();
                        TokenKind::LessEq
                    } else {
                        TokenKind::Less
                    };
                    let len = self.cursor.saturating_sub(offset);
                    self.tokens.push(Token {
                        kind,
                        span: Span::new(line, column, offset, len),
                    });
                }
                '>' => {
                    self.advance();
                    let kind = if self.peek() == Some('=') {
                        self.advance();
                        TokenKind::GreaterEq
                    } else {
                        TokenKind::Greater
                    };
                    let len = self.cursor.saturating_sub(offset);
                    self.tokens.push(Token {
                        kind,
                        span: Span::new(line, column, offset, len),
                    });
                }
                '&' => {
                    self.advance();
                    if self.peek() == Some('&') {
                        self.advance();
                        self.tokens.push(Token {
                            kind: TokenKind::AmpAmp,
                            span: Span::new(line, column, offset, 2),
                        });
                    } else {
                        self.diagnostics.push(
                            Diagnostic::error("AERIS-LEX-001", "unexpected '&'")
                                .with_span(Span::new(line, column, offset, 1))
                                .with_help("use '&&' for logical conjunction"),
                        );
                    }
                }
                '|' => {
                    self.advance();
                    let kind = if self.peek() == Some('|') {
                        self.advance();
                        TokenKind::PipePipe
                    } else {
                        TokenKind::Pipe
                    };
                    let len = self.cursor.saturating_sub(offset);
                    self.tokens.push(Token {
                        kind,
                        span: Span::new(line, column, offset, len),
                    });
                }
                '/' => self.push_simple(TokenKind::Slash, line, column, offset),
                other => {
                    self.advance();
                    self.diagnostics.push(
                        Diagnostic::error(
                            "AERIS-LEX-002",
                            format!("unexpected character '{other}'"),
                        )
                        .with_span(Span::new(line, column, offset, 1)),
                    );
                }
            }
        }

        self.tokens.push(Token {
            kind: TokenKind::Eof,
            span: Span::new(self.line, self.column, self.cursor, 0),
        });
    }

    fn push_simple(&mut self, kind: TokenKind, line: usize, column: usize, offset: usize) {
        self.advance();
        self.tokens.push(Token {
            kind,
            span: Span::new(line, column, offset, 1),
        });
    }

    fn lex_ident_or_keyword(&mut self, line: usize, column: usize, offset: usize) {
        let mut value = String::new();
        while let Some(ch) = self.peek() {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                value.push(ch);
                self.advance();
            } else {
                break;
            }
        }
        let kind = match value.as_str() {
            "module" => TokenKind::Module,
            "use" => TokenKind::Use,
            "as" => TokenKind::As,
            "effect" => TokenKind::Effect,
            "cap" => TokenKind::Cap,
            "type" => TokenKind::Type,
            "fn" => TokenKind::Fn,
            "actor" => TokenKind::Actor,
            "on" => TokenKind::On,
            "where" => TokenKind::Where,
            "let" => TokenKind::Let,
            "mut" => TokenKind::Mut,
            "require" => TokenKind::Require,
            "return" => TokenKind::Return,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "match" => TokenKind::Match,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            _ => TokenKind::Ident(value),
        };
        self.tokens.push(Token {
            kind,
            span: Span::new(line, column, offset, self.cursor.saturating_sub(offset)),
        });
    }

    fn lex_number(&mut self, line: usize, column: usize, offset: usize) {
        let mut value = String::new();
        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() {
                value.push(ch);
                self.advance();
            } else {
                break;
            }
        }
        match value.parse::<i64>() {
            Ok(parsed) => self.tokens.push(Token {
                kind: TokenKind::Int(parsed),
                span: Span::new(line, column, offset, self.cursor.saturating_sub(offset)),
            }),
            Err(_) => self.diagnostics.push(
                Diagnostic::error("AERIS-LEX-003", "integer literal overflow").with_span(
                    Span::new(line, column, offset, self.cursor.saturating_sub(offset)),
                ),
            ),
        }
    }

    fn lex_string(&mut self, line: usize, column: usize, offset: usize) {
        self.advance();
        let mut value = String::new();
        let mut terminated = false;
        while let Some(ch) = self.peek() {
            self.advance();
            match ch {
                '"' => {
                    terminated = true;
                    break;
                }
                '\\' => {
                    if let Some(escaped) = self.peek() {
                        self.advance();
                        let mapped = match escaped {
                            'n' => '\n',
                            't' => '\t',
                            '"' => '"',
                            '\\' => '\\',
                            other => other,
                        };
                        value.push(mapped);
                    }
                }
                other => value.push(other),
            }
        }

        if terminated {
            self.tokens.push(Token {
                kind: TokenKind::Text(value),
                span: Span::new(line, column, offset, self.cursor.saturating_sub(offset)),
            });
        } else {
            self.diagnostics.push(
                Diagnostic::error("AERIS-LEX-004", "unterminated string literal").with_span(
                    Span::new(line, column, offset, self.cursor.saturating_sub(offset)),
                ),
            );
        }
    }

    fn consume_whitespace(&mut self) {
        while let Some(ch) = self.peek() {
            if ch.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn consume_comment(&mut self) {
        while let Some(ch) = self.peek() {
            self.advance();
            if ch == '\n' {
                break;
            }
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.cursor).copied()
    }

    fn peek_next(&self) -> Option<char> {
        self.chars.get(self.cursor + 1).copied()
    }

    fn advance(&mut self) {
        if let Some(ch) = self.chars.get(self.cursor).copied() {
            self.cursor += 1;
            if ch == '\n' {
                self.line += 1;
                self.column = 1;
            } else {
                self.column += 1;
            }
        }
    }
}

#[allow(dead_code)]
fn _source_len(source: &str) -> usize {
    source.len()
}
