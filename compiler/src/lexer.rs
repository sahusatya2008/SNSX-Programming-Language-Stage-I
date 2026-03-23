use crate::diag::Span;

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Ident(String),
    Int(i64),
    Float(f64),
    String(String),
    Newline,
    Indent,
    Dedent,
    LParen,
    RParen,
    LBracket,
    RBracket,
    Colon,
    Comma,
    Dot,
    PipeForward,
    Arrow,
    FatArrow,
    Assign,
    EqEq,
    NotEq,
    Less,
    LessEq,
    Greater,
    GreaterEq,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Underscore,
    KwUse,
    KwFn,
    KwAi,
    KwAsync,
    KwActor,
    KwState,
    KwLet,
    KwMut,
    KwIf,
    KwUnless,
    KwElse,
    KwWhile,
    KwReturn,
    KwMatch,
    KwTrue,
    KwFalse,
    KwAwait,
    KwSpawn,
    KwParallel,
    KwMove,
    KwPrompt,
    Eof,
}

pub fn lex(source: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut indent_stack = vec![0usize];
    let mut offset = 0usize;

    for raw_line in source.split_inclusive('\n') {
        let line = raw_line.strip_suffix('\n').unwrap_or(raw_line);
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            offset += raw_line.len();
            continue;
        }

        let indent = line.chars().take_while(|c| *c == ' ').count();
        let content_start = offset + indent;

        if indent > *indent_stack.last().unwrap() {
            indent_stack.push(indent);
            tokens.push(Token {
                kind: TokenKind::Indent,
                span: Span {
                    start: offset,
                    end: content_start,
                },
            });
        } else {
            while indent < *indent_stack.last().unwrap() {
                indent_stack.pop();
                tokens.push(Token {
                    kind: TokenKind::Dedent,
                    span: Span {
                        start: offset,
                        end: offset,
                    },
                });
            }
        }

        lex_line(&line[indent..], content_start, &mut tokens);
        tokens.push(Token {
            kind: TokenKind::Newline,
            span: Span {
                start: offset + line.len(),
                end: offset + line.len(),
            },
        });
        offset += raw_line.len();
    }

    while indent_stack.len() > 1 {
        indent_stack.pop();
        tokens.push(Token {
            kind: TokenKind::Dedent,
            span: Span {
                start: offset,
                end: offset,
            },
        });
    }

    tokens.push(Token {
        kind: TokenKind::Eof,
        span: Span {
            start: offset,
            end: offset,
        },
    });

    tokens
}

fn lex_line(line: &str, absolute_start: usize, tokens: &mut Vec<Token>) {
    let bytes = line.as_bytes();
    let mut idx = 0usize;
    while idx < bytes.len() {
        let ch = bytes[idx] as char;
        if ch == ' ' || ch == '\t' {
            idx += 1;
            continue;
        }
        if ch == '#' {
            break;
        }

        let start = absolute_start + idx;
        let token = match ch {
            '(' => simple(TokenKind::LParen, start),
            ')' => simple(TokenKind::RParen, start),
            '[' => simple(TokenKind::LBracket, start),
            ']' => simple(TokenKind::RBracket, start),
            ':' => simple(TokenKind::Colon, start),
            ',' => simple(TokenKind::Comma, start),
            '.' => simple(TokenKind::Dot, start),
            '|' => {
                if idx + 1 < bytes.len() && bytes[idx + 1] as char == '>' {
                    Token {
                        kind: TokenKind::PipeForward,
                        span: Span {
                            start,
                            end: start + 2,
                        },
                    }
                } else {
                    idx += 1;
                    continue;
                }
            }
            '+' => simple(TokenKind::Plus, start),
            '*' => simple(TokenKind::Star, start),
            '/' => simple(TokenKind::Slash, start),
            '%' => simple(TokenKind::Percent, start),
            '_' => simple(TokenKind::Underscore, start),
            '-' => {
                if idx + 1 < bytes.len() && bytes[idx + 1] as char == '>' {
                    Token {
                        kind: TokenKind::Arrow,
                        span: Span {
                            start,
                            end: start + 2,
                        },
                    }
                } else {
                    simple(TokenKind::Minus, start)
                }
            }
            '=' => {
                if idx + 1 < bytes.len() && bytes[idx + 1] as char == '>' {
                    Token {
                        kind: TokenKind::FatArrow,
                        span: Span {
                            start,
                            end: start + 2,
                        },
                    }
                } else if idx + 1 < bytes.len() && bytes[idx + 1] as char == '=' {
                    Token {
                        kind: TokenKind::EqEq,
                        span: Span {
                            start,
                            end: start + 2,
                        },
                    }
                } else {
                    simple(TokenKind::Assign, start)
                }
            }
            '!' => {
                if idx + 1 < bytes.len() && bytes[idx + 1] as char == '=' {
                    Token {
                        kind: TokenKind::NotEq,
                        span: Span {
                            start,
                            end: start + 2,
                        },
                    }
                } else {
                    idx += 1;
                    continue;
                }
            }
            '<' => {
                if idx + 1 < bytes.len() && bytes[idx + 1] as char == '=' {
                    Token {
                        kind: TokenKind::LessEq,
                        span: Span {
                            start,
                            end: start + 2,
                        },
                    }
                } else {
                    simple(TokenKind::Less, start)
                }
            }
            '>' => {
                if idx + 1 < bytes.len() && bytes[idx + 1] as char == '=' {
                    Token {
                        kind: TokenKind::GreaterEq,
                        span: Span {
                            start,
                            end: start + 2,
                        },
                    }
                } else {
                    simple(TokenKind::Greater, start)
                }
            }
            '"' => lex_string(line, absolute_start, idx),
            c if c.is_ascii_digit() => lex_number(line, absolute_start, idx),
            c if is_ident_start(c) => lex_ident(line, absolute_start, idx),
            _ => {
                idx += 1;
                continue;
            }
        };
        tokens.push(token);
        idx = tokens
            .last()
            .map(|token| token.span.end.saturating_sub(absolute_start))
            .unwrap_or(idx + 1);
    }
}

fn simple(kind: TokenKind, start: usize) -> Token {
    Token {
        kind,
        span: Span {
            start,
            end: start + 1,
        },
    }
}

fn lex_string(line: &str, absolute_start: usize, idx: usize) -> Token {
    let bytes = line.as_bytes();
    let mut end = idx + 1;
    let mut escaped = false;
    let mut buffer = String::new();
    while end < bytes.len() {
        let ch = bytes[end] as char;
        if escaped {
            buffer.push(match ch {
                'n' => '\n',
                't' => '\t',
                '"' => '"',
                '\\' => '\\',
                other => other,
            });
            escaped = false;
            end += 1;
            continue;
        }
        match ch {
            '\\' => {
                escaped = true;
                end += 1;
            }
            '"' => {
                end += 1;
                break;
            }
            other => {
                buffer.push(other);
                end += 1;
            }
        }
    }
    Token {
        kind: TokenKind::String(buffer),
        span: Span {
            start: absolute_start + idx,
            end: absolute_start + end,
        },
    }
}

fn lex_number(line: &str, absolute_start: usize, idx: usize) -> Token {
    let bytes = line.as_bytes();
    let mut end = idx;
    let mut saw_dot = false;
    while end < bytes.len() {
        let ch = bytes[end] as char;
        if ch.is_ascii_digit() {
            end += 1;
            continue;
        }
        if ch == '.' && !saw_dot {
            saw_dot = true;
            end += 1;
            continue;
        }
        break;
    }
    let raw = &line[idx..end];
    if saw_dot {
        Token {
            kind: TokenKind::Float(raw.parse().unwrap_or(0.0)),
            span: Span {
                start: absolute_start + idx,
                end: absolute_start + end,
            },
        }
    } else {
        Token {
            kind: TokenKind::Int(raw.parse().unwrap_or(0)),
            span: Span {
                start: absolute_start + idx,
                end: absolute_start + end,
            },
        }
    }
}

fn lex_ident(line: &str, absolute_start: usize, idx: usize) -> Token {
    let bytes = line.as_bytes();
    let mut end = idx;
    while end < bytes.len() {
        let ch = bytes[end] as char;
        if is_ident_continue(ch) {
            end += 1;
        } else {
            break;
        }
    }
    let raw = &line[idx..end];
    let kind = match raw {
        "use" => TokenKind::KwUse,
        "need" => TokenKind::KwUse,
        "bring" => TokenKind::KwUse,
        "fn" => TokenKind::KwFn,
        "flow" => TokenKind::KwFn,
        "ai" => TokenKind::KwAi,
        "mind" => TokenKind::KwAi,
        "async" => TokenKind::KwAsync,
        "later" => TokenKind::KwAsync,
        "actor" => TokenKind::KwActor,
        "agent" => TokenKind::KwActor,
        "state" => TokenKind::KwState,
        "memory" => TokenKind::KwState,
        "let" => TokenKind::KwLet,
        "make" => TokenKind::KwLet,
        "hold" => TokenKind::KwLet,
        "mut" => TokenKind::KwMut,
        "change" => TokenKind::KwMut,
        "if" => TokenKind::KwIf,
        "when" => TokenKind::KwIf,
        "gate" => TokenKind::KwIf,
        "unless" => TokenKind::KwUnless,
        "else" => TokenKind::KwElse,
        "otherwise" => TokenKind::KwElse,
        "while" => TokenKind::KwWhile,
        "during" => TokenKind::KwWhile,
        "return" => TokenKind::KwReturn,
        "send" => TokenKind::KwReturn,
        "give" => TokenKind::KwReturn,
        "match" => TokenKind::KwMatch,
        "choose" => TokenKind::KwMatch,
        "pick" => TokenKind::KwMatch,
        "true" => TokenKind::KwTrue,
        "false" => TokenKind::KwFalse,
        "await" => TokenKind::KwAwait,
        "wait" => TokenKind::KwAwait,
        "spawn" => TokenKind::KwSpawn,
        "launch" => TokenKind::KwSpawn,
        "parallel" => TokenKind::KwParallel,
        "mesh" => TokenKind::KwParallel,
        "move" => TokenKind::KwMove,
        "carry" => TokenKind::KwMove,
        "prompt" => TokenKind::KwPrompt,
        "from" => TokenKind::KwPrompt,
        _ => TokenKind::Ident(raw.to_string()),
    };
    Token {
        kind,
        span: Span {
            start: absolute_start + idx,
            end: absolute_start + end,
        },
    }
}

fn is_ident_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

fn is_ident_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}
