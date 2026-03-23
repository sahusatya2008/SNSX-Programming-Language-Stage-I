use crate::ast::*;
use crate::diag::{Diagnostic, Span};
use crate::lexer::{Token, TokenKind};

pub struct Parser<'a> {
    path: &'a str,
    _source: &'a str,
    tokens: Vec<Token>,
    pos: usize,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Parser<'a> {
    pub fn new(path: &'a str, source: &'a str, tokens: Vec<Token>) -> Self {
        Self {
            path,
            _source: source,
            tokens,
            pos: 0,
            diagnostics: Vec::new(),
        }
    }

    pub fn parse_module(&mut self) -> Result<Module, Vec<Diagnostic>> {
        let start = self.current_span().start;
        let mut uses = Vec::new();
        let mut items = Vec::new();
        while !self.is_eof() {
            self.skip_newlines();
            if self.is_eof() {
                break;
            }
            match self.current_kind() {
                Some(TokenKind::KwUse) => uses.push(self.parse_use()),
                Some(TokenKind::Ident(name)) if name == "bundle" => {
                    uses.extend(self.parse_bundle_uses())
                }
                Some(TokenKind::Ident(name)) if name == "entry" => {
                    items.push(Item::Function(self.parse_entry()))
                }
                Some(TokenKind::KwFn) | Some(TokenKind::KwAi) | Some(TokenKind::KwAsync) => {
                    items.push(Item::Function(self.parse_function()))
                }
                Some(TokenKind::KwActor) => items.push(Item::Actor(self.parse_actor())),
                _ => {
                    self.error_here("expected top-level declaration");
                    self.bump();
                }
            }
        }

        if self.diagnostics.is_empty() {
            Ok(Module {
                path: self.path.to_string(),
                uses,
                items,
                span: Span {
                    start,
                    end: self.current_span().end,
                },
            })
        } else {
            Err(std::mem::take(&mut self.diagnostics))
        }
    }

    fn parse_use(&mut self) -> UseDecl {
        let start = self.expect(TokenKind::KwUse).start;
        let path = self.parse_import_path();
        let alias = self.parse_import_alias();
        let end = self.consume_newline().unwrap_or(self.current_span()).end;
        UseDecl {
            path,
            alias,
            span: Span { start, end },
        }
    }

    fn parse_bundle_uses(&mut self) -> Vec<UseDecl> {
        let bundle_start = self.current_span().start;
        self.expect_named_ident("bundle");
        let mut uses = Vec::new();
        loop {
            let start = self.current_span().start.max(bundle_start);
            let path = self.parse_import_path();
            let alias = self.parse_import_alias();
            uses.push(UseDecl {
                path,
                alias,
                span: Span {
                    start,
                    end: self.previous_span().end,
                },
            });
            if !self.consume(TokenKind::Comma) {
                break;
            }
        }
        self.consume_newline();
        uses
    }

    fn parse_import_path(&mut self) -> Vec<String> {
        if self.consume(TokenKind::Less) {
            let head = self.expect_ident();
            let mut path = Vec::new();
            if self.consume(TokenKind::Slash) {
                path = self.parse_import_segments_until(TokenKind::Greater);
                if path.is_empty() {
                    self.error_here("expected module, lib, or package path inside bring <...>");
                }
                if !matches!(head.as_str(), "module" | "lib" | "package") {
                    path.insert(0, head);
                }
            } else {
                path.push(head);
                while self.consume(TokenKind::Dot) || self.consume(TokenKind::Slash) {
                    path.push(self.expect_ident());
                }
            }
            self.expect(TokenKind::Greater);
            return path;
        }

        let mut path = Vec::new();
        loop {
            match self.bump_path_segment() {
                Some(name) => path.push(name),
                None => {
                    let other = self.previous_kind();
                    self.diagnostics.push(Diagnostic::error(
                        format!("expected identifier in import path, found {:?}", other),
                        Some(self.current_span()),
                    ));
                    break;
                }
            }
            if !self.consume(TokenKind::Dot) {
                break;
            }
        }
        path
    }

    fn parse_import_segments_until(&mut self, terminator: TokenKind) -> Vec<String> {
        let mut path = Vec::new();
        while !self.check(terminator.clone()) && !self.is_eof() {
            match self.bump_path_segment() {
                Some(name) => path.push(name),
                None => {
                    let other = self.previous_kind();
                    self.diagnostics.push(Diagnostic::error(
                        format!("expected identifier in import path, found {:?}", other),
                        Some(self.current_span()),
                    ));
                    break;
                }
            }
            if !(self.consume(TokenKind::Dot) || self.consume(TokenKind::Slash)) {
                break;
            }
        }
        path
    }

    fn parse_import_alias(&mut self) -> Option<String> {
        if self.current_ident_is("as") {
            self.expect_named_ident("as");
            Some(self.expect_alias_ident())
        } else {
            None
        }
    }

    fn parse_function(&mut self) -> Function {
        let start = self.current_span().start;
        let mut is_async = false;
        let mut is_ai = false;
        loop {
            match self.current_kind() {
                Some(TokenKind::KwAsync) => {
                    is_async = true;
                    self.bump();
                }
                Some(TokenKind::KwAi) => {
                    is_ai = true;
                    self.bump();
                }
                _ => break,
            }
        }
        if !self.consume(TokenKind::KwFn) && !is_async && !is_ai {
            self.expect(TokenKind::KwFn);
        }
        let name = self.expect_ident();
        let (params, return_type, prompt, body) = if self.consume(TokenKind::LParen) {
            let params = self.parse_param_list();
            self.expect(TokenKind::RParen);
            let return_type = self.parse_optional_return_type();
            let prompt = self.parse_optional_prompt();
            let body = if self.consume(TokenKind::Colon) {
                self.parse_block()
            } else if self.check(TokenKind::Newline)
                && matches!(self.peek_kind(), Some(TokenKind::Indent))
            {
                self.parse_block()
            } else if is_ai && self.check(TokenKind::Newline) {
                self.bump();
                Vec::new()
            } else {
                self.expect(TokenKind::Colon);
                self.parse_block()
            };
            (params, return_type, prompt, body)
        } else if self.check(TokenKind::Newline)
            && matches!(self.peek_kind(), Some(TokenKind::Indent))
        {
            let (params, return_type, prompt, body) = self.parse_structured_function_block();
            (params, return_type, prompt, body)
        } else {
            let return_type = self.parse_optional_return_type();
            let prompt = self.parse_optional_prompt();
            let body = if self.consume(TokenKind::Colon) {
                self.parse_block()
            } else if self.check(TokenKind::Newline)
                && matches!(self.peek_kind(), Some(TokenKind::Indent))
            {
                self.parse_block()
            } else if is_ai && self.check(TokenKind::Newline) {
                self.bump();
                Vec::new()
            } else {
                self.expect(TokenKind::Colon);
                self.parse_block()
            };
            (Vec::new(), return_type, prompt, body)
        };
        let body = self.desugar_tail_return(body, return_type.as_ref());
        Function {
            name,
            params,
            return_type,
            body,
            is_async,
            is_ai,
            prompt,
            span: Span {
                start,
                end: self.previous_span().end,
            },
        }
    }

    fn parse_entry(&mut self) -> Function {
        let start = self.current_span().start;
        self.expect_named_ident("entry");
        let return_type = if self.consume(TokenKind::Arrow) {
            self.parse_type()
        } else {
            TypeExpr {
                kind: TypeExprKind::Path(vec!["Int".to_string()]),
                span: Span { start, end: start },
            }
        };
        if self.consume(TokenKind::Colon) {
            let body = self.parse_block();
            let body = self.desugar_tail_return(body, Some(&return_type));
            return Function {
                name: "main".to_string(),
                params: Vec::new(),
                return_type: Some(return_type),
                body,
                is_async: false,
                is_ai: false,
                prompt: None,
                span: Span {
                    start,
                    end: self.previous_span().end,
                },
            };
        }
        let body = self.parse_block();
        let body = self.desugar_tail_return(body, Some(&return_type));
        Function {
            name: "main".to_string(),
            params: Vec::new(),
            return_type: Some(return_type),
            body,
            is_async: false,
            is_ai: false,
            prompt: None,
            span: Span {
                start,
                end: self.previous_span().end,
            },
        }
    }

    fn parse_param_list(&mut self) -> Vec<Param> {
        let mut params = Vec::new();
        while !self.check(TokenKind::RParen) && !self.is_eof() {
            let param_start = self.current_span().start;
            let param_name = self.expect_ident();
            let ty = if self.consume(TokenKind::Colon) {
                Some(self.parse_type())
            } else {
                None
            };
            let param_end = ty
                .as_ref()
                .map(|t| t.span.end)
                .unwrap_or(self.previous_span().end);
            params.push(Param {
                name: param_name,
                ty,
                span: Span {
                    start: param_start,
                    end: param_end,
                },
            });
            if !self.consume(TokenKind::Comma) {
                break;
            }
        }
        params
    }

    fn parse_optional_return_type(&mut self) -> Option<TypeExpr> {
        if self.consume(TokenKind::Arrow) {
            Some(self.parse_type())
        } else {
            None
        }
    }

    fn parse_optional_prompt(&mut self) -> Option<String> {
        if self.consume(TokenKind::KwPrompt) {
            match self.bump_kind() {
                TokenKind::String(text) => Some(text),
                other => {
                    self.diagnostics.push(Diagnostic::error(
                        format!("expected prompt string, found {:?}", other),
                        Some(self.previous_span()),
                    ));
                    None
                }
            }
        } else {
            None
        }
    }

    fn parse_structured_function_block(
        &mut self,
    ) -> (Vec<Param>, Option<TypeExpr>, Option<String>, Vec<Stmt>) {
        let _ = self.parse_block_raw();
        let mut params = Vec::new();
        let mut return_type = None;
        let mut prompt = None;
        let mut body = Vec::new();
        let mut seen_body = false;

        while !self.check(TokenKind::Dedent) && !self.is_eof() {
            self.skip_newlines();
            if self.check(TokenKind::Dedent) || self.is_eof() {
                break;
            }

            if !seen_body && self.current_ident_is("takes") {
                self.expect_named_ident("takes");
                if !params.is_empty() {
                    self.error_here("duplicate takes clause");
                }
                params = self.parse_clause_params();
                self.consume_newline();
                continue;
            }
            if !seen_body && self.current_ident_is("gives") {
                self.expect_named_ident("gives");
                if return_type.is_some() {
                    self.error_here("duplicate gives clause");
                }
                return_type = Some(self.parse_type());
                self.consume_newline();
                continue;
            }
            if !seen_body && self.check(TokenKind::KwPrompt) {
                self.bump();
                if prompt.is_some() {
                    self.error_here("duplicate from clause");
                }
                prompt = match self.bump_kind() {
                    TokenKind::String(text) => Some(text),
                    other => {
                        self.diagnostics.push(Diagnostic::error(
                            format!("expected prompt string, found {:?}", other),
                            Some(self.previous_span()),
                        ));
                        None
                    }
                };
                self.consume_newline();
                continue;
            }

            seen_body = true;
            body.push(self.parse_stmt());
            self.skip_newlines();
        }

        self.expect(TokenKind::Dedent);
        (params, return_type, prompt, body)
    }

    fn parse_clause_params(&mut self) -> Vec<Param> {
        let mut params = Vec::new();
        while !self.check(TokenKind::Newline) && !self.is_eof() {
            let param_start = self.current_span().start;
            let name = self.expect_ident();
            let ty = if self.consume(TokenKind::Colon) {
                Some(self.parse_type())
            } else {
                None
            };
            let param_end = ty
                .as_ref()
                .map(|t| t.span.end)
                .unwrap_or(self.previous_span().end);
            params.push(Param {
                name,
                ty,
                span: Span {
                    start: param_start,
                    end: param_end,
                },
            });
            if !self.consume(TokenKind::Comma) {
                break;
            }
        }
        params
    }

    fn parse_actor(&mut self) -> Actor {
        let start = self.expect(TokenKind::KwActor).start;
        let name = self.expect_ident();
        self.expect(TokenKind::Colon);
        let block_start = self.parse_block_raw();
        let mut state = Vec::new();
        let mut methods = Vec::new();
        while !self.check(TokenKind::Dedent) && !self.is_eof() {
            self.skip_newlines();
            match self.current_kind() {
                Some(TokenKind::KwState) => {
                    let field_start = self.bump().span.start;
                    let field_name = self.expect_ident();
                    self.expect(TokenKind::Colon);
                    let ty = self.parse_type();
                    let init = if self.consume(TokenKind::Assign) {
                        Some(self.parse_expr())
                    } else {
                        None
                    };
                    let span = Span {
                        start: field_start,
                        end: self.previous_span().end,
                    };
                    self.consume_newline();
                    state.push(StateField {
                        name: field_name,
                        ty,
                        init,
                        span,
                    });
                }
                Some(TokenKind::KwFn) | Some(TokenKind::KwAi) | Some(TokenKind::KwAsync) => {
                    methods.push(self.parse_function())
                }
                _ => {
                    self.error_here("expected actor field or method");
                    self.bump();
                }
            }
            self.skip_newlines();
        }
        self.expect(TokenKind::Dedent);
        Actor {
            name,
            state,
            methods,
            span: Span {
                start,
                end: block_start.end.max(self.previous_span().end),
            },
        }
    }

    fn parse_block(&mut self) -> Vec<Stmt> {
        let _ = self.parse_block_raw();
        let mut body = Vec::new();
        while !self.check(TokenKind::Dedent) && !self.is_eof() {
            self.skip_newlines();
            if self.check(TokenKind::Dedent) || self.is_eof() {
                break;
            }
            body.push(self.parse_stmt());
            self.skip_newlines();
        }
        self.expect(TokenKind::Dedent);
        body
    }

    fn parse_block_raw(&mut self) -> Span {
        let nl = self.expect(TokenKind::Newline);
        let ind = self.expect(TokenKind::Indent);
        Span {
            start: nl.start,
            end: ind.end,
        }
    }

    fn parse_stmt(&mut self) -> Stmt {
        match self.current_kind() {
            Some(TokenKind::Ident(name))
                if name == "show"
                    && !self.peek_is(TokenKind::Assign)
                    && !self.peek_is(TokenKind::LParen) =>
            {
                self.parse_show_stmt()
            }
            Some(TokenKind::Ident(name)) if name == "set" => self.parse_set_stmt(),
            Some(TokenKind::KwLet) => self.parse_let_stmt(),
            Some(TokenKind::KwReturn) => self.parse_return_stmt(),
            Some(TokenKind::KwIf) => self.parse_if_stmt(),
            Some(TokenKind::KwUnless) => self.parse_unless_stmt(),
            Some(TokenKind::KwWhile) => self.parse_while_stmt(),
            Some(TokenKind::KwMatch) => self.parse_match_stmt(),
            Some(TokenKind::Ident(name)) if name == "guard" => self.parse_guard_stmt(),
            Some(TokenKind::Ident(name)) if name == "fail" => self.parse_fail_stmt(),
            Some(TokenKind::Ident(_)) if self.peek_is(TokenKind::Assign) => {
                self.parse_assign_stmt()
            }
            _ => {
                let expr = self.parse_expr();
                self.consume_newline();
                Stmt::Expr(expr)
            }
        }
    }

    fn parse_let_stmt(&mut self) -> Stmt {
        let start = self.expect(TokenKind::KwLet).start;
        let mutable = self.consume(TokenKind::KwMut);
        let name = self.expect_ident();
        let ty = if self.consume(TokenKind::Colon) {
            Some(self.parse_type())
        } else {
            None
        };
        self.expect(TokenKind::Assign);
        let expr = self.parse_expr();
        self.consume_newline();
        Stmt::Let {
            name,
            ty,
            expr,
            mutable,
            span: Span {
                start,
                end: self.previous_span().end,
            },
        }
    }

    fn parse_assign_stmt(&mut self) -> Stmt {
        let start = self.current_span().start;
        let name = self.expect_ident();
        self.expect(TokenKind::Assign);
        let expr = self.parse_expr();
        self.consume_newline();
        Stmt::Assign {
            target: name,
            expr,
            span: Span {
                start,
                end: self.previous_span().end,
            },
        }
    }

    fn parse_show_stmt(&mut self) -> Stmt {
        let start = self.current_span().start;
        self.expect_named_ident("show");
        let expr = self.parse_expr();
        let expr_end = expr.span().end;
        self.consume_newline();
        Stmt::Expr(Expr::Call {
            callee: Box::new(Expr::Ident("print".to_string(), Span { start, end: start })),
            args: vec![expr],
            span: Span {
                start,
                end: expr_end,
            },
        })
    }

    fn parse_set_stmt(&mut self) -> Stmt {
        let start = self.current_span().start;
        self.expect_named_ident("set");
        let target = self.expect_ident();
        self.expect(TokenKind::Assign);
        let expr = self.parse_expr();
        self.consume_newline();
        Stmt::Assign {
            target,
            expr,
            span: Span {
                start,
                end: self.previous_span().end,
            },
        }
    }

    fn parse_return_stmt(&mut self) -> Stmt {
        let start = self.expect(TokenKind::KwReturn).start;
        if self.check(TokenKind::Newline) {
            self.bump();
            return Stmt::Return {
                expr: None,
                span: Span {
                    start,
                    end: self.previous_span().end,
                },
            };
        }
        let expr = self.parse_expr();
        self.consume_newline();
        Stmt::Return {
            expr: Some(expr),
            span: Span {
                start,
                end: self.previous_span().end,
            },
        }
    }

    fn parse_if_stmt(&mut self) -> Stmt {
        let start = self.expect(TokenKind::KwIf).start;
        let condition = self.parse_expr();
        self.consume(TokenKind::Colon);
        let then_branch = self.parse_block();
        let else_branch = if self.consume(TokenKind::KwElse) {
            self.consume(TokenKind::Colon);
            self.parse_block()
        } else {
            Vec::new()
        };
        Stmt::If {
            condition,
            then_branch,
            else_branch,
            span: Span {
                start,
                end: self.previous_span().end,
            },
        }
    }

    fn parse_unless_stmt(&mut self) -> Stmt {
        let start = self.expect(TokenKind::KwUnless).start;
        let condition = self.parse_expr();
        let condition_span = condition.span();
        self.consume(TokenKind::Colon);
        let then_branch = self.parse_block();
        let else_branch = if self.consume(TokenKind::KwElse) {
            self.consume(TokenKind::Colon);
            self.parse_block()
        } else {
            Vec::new()
        };
        Stmt::If {
            condition: Expr::Unary {
                op: UnaryOp::Not,
                expr: Box::new(condition),
                span: Span {
                    start,
                    end: condition_span.end,
                },
            },
            then_branch,
            else_branch,
            span: Span {
                start,
                end: self.previous_span().end,
            },
        }
    }

    fn parse_while_stmt(&mut self) -> Stmt {
        let start = self.expect(TokenKind::KwWhile).start;
        let condition = self.parse_expr();
        self.consume(TokenKind::Colon);
        let body = self.parse_block();
        Stmt::While {
            condition,
            body,
            span: Span {
                start,
                end: self.previous_span().end,
            },
        }
    }

    fn parse_match_stmt(&mut self) -> Stmt {
        let start = self.expect(TokenKind::KwMatch).start;
        let expr = self.parse_expr();
        self.consume(TokenKind::Colon);
        self.parse_block_raw();
        let mut arms = Vec::new();
        while !self.check(TokenKind::Dedent) && !self.is_eof() {
            self.skip_newlines();
            if self.check(TokenKind::Dedent) {
                break;
            }
            let (pattern, body) = if self.current_ident_is("case") {
                self.expect_named_ident("case");
                let pattern = self.parse_pattern();
                let body = if self.check(TokenKind::Newline) {
                    self.parse_block()
                } else {
                    self.parse_inline_arm_body()
                };
                (pattern, body)
            } else if self.check(TokenKind::KwElse) {
                let start = self.bump().span.start;
                let pattern = Pattern::Wildcard(Span {
                    start,
                    end: self.previous_span().end,
                });
                let body = if self.check(TokenKind::Newline) {
                    self.parse_block()
                } else {
                    self.parse_inline_arm_body()
                };
                (pattern, body)
            } else {
                let pattern = self.parse_pattern();
                self.expect(TokenKind::FatArrow);
                let body = if self.check(TokenKind::Newline) {
                    self.parse_block()
                } else {
                    self.parse_inline_arm_body()
                };
                (pattern, body)
            };
            let span = Span {
                start: pattern.span().start,
                end: self.previous_span().end,
            };
            arms.push(MatchArm {
                pattern,
                body,
                span,
            });
        }
        self.expect(TokenKind::Dedent);
        Stmt::Match {
            expr,
            arms,
            span: Span {
                start,
                end: self.previous_span().end,
            },
        }
    }

    fn parse_guard_stmt(&mut self) -> Stmt {
        let start = self.current_span().start;
        self.expect_named_ident("guard");
        let condition = self.parse_expr();
        self.expect(TokenKind::Comma);
        let message = self.parse_expr();
        let end = message.span().end;
        self.consume_newline();
        Stmt::Expr(Expr::Call {
            callee: Box::new(Expr::Ident("guard".to_string(), Span { start, end: start })),
            args: vec![condition, message],
            span: Span { start, end },
        })
    }

    fn parse_fail_stmt(&mut self) -> Stmt {
        let start = self.current_span().start;
        self.expect_named_ident("fail");
        let message = self.parse_expr();
        let end = message.span().end;
        self.consume_newline();
        Stmt::Expr(Expr::Call {
            callee: Box::new(Expr::Ident("fail".to_string(), Span { start, end: start })),
            args: vec![message],
            span: Span { start, end },
        })
    }

    fn parse_pattern(&mut self) -> Pattern {
        match self.bump_kind() {
            TokenKind::Underscore => Pattern::Wildcard(self.previous_span()),
            TokenKind::Int(value) => Pattern::Int(value, self.previous_span()),
            TokenKind::Float(value) => Pattern::Float(value, self.previous_span()),
            TokenKind::KwTrue => Pattern::Bool(true, self.previous_span()),
            TokenKind::KwFalse => Pattern::Bool(false, self.previous_span()),
            TokenKind::String(value) => Pattern::String(value, self.previous_span()),
            TokenKind::Ident(name) => Pattern::Ident(name, self.previous_span()),
            other => {
                self.diagnostics.push(Diagnostic::error(
                    format!("unsupported pattern token {:?}", other),
                    Some(self.previous_span()),
                ));
                Pattern::Wildcard(self.previous_span())
            }
        }
    }

    fn parse_inline_arm_body(&mut self) -> Vec<Stmt> {
        if (self.current_ident_is("show")
            && !self.peek_is(TokenKind::Assign)
            && !self.peek_is(TokenKind::LParen))
            || self.current_ident_is("set")
            || self.current_ident_is("guard")
            || self.current_ident_is("fail")
        {
            return vec![self.parse_stmt()];
        }
        if matches!(self.current_kind(), Some(TokenKind::Ident(_)))
            && self.peek_is(TokenKind::Assign)
        {
            return vec![self.parse_stmt()];
        }
        match self.current_kind() {
            Some(TokenKind::KwLet)
            | Some(TokenKind::KwReturn)
            | Some(TokenKind::KwIf)
            | Some(TokenKind::KwUnless)
            | Some(TokenKind::KwWhile)
            | Some(TokenKind::KwMatch) => vec![self.parse_stmt()],
            _ => {
                let expr = self.parse_expr();
                self.consume_newline();
                vec![Stmt::Expr(expr)]
            }
        }
    }

    fn parse_type(&mut self) -> TypeExpr {
        let start = self.current_span().start;
        let mut base = vec![self.expect_ident()];
        while self.consume(TokenKind::Dot) {
            base.push(self.expect_ident());
        }
        let kind = if self.consume(TokenKind::LBracket) {
            let mut args = Vec::new();
            while !self.check(TokenKind::RBracket) && !self.is_eof() {
                args.push(self.parse_type());
                if !self.consume(TokenKind::Comma) {
                    break;
                }
            }
            self.expect(TokenKind::RBracket);
            TypeExprKind::Generic { base, args }
        } else {
            TypeExprKind::Path(base)
        };
        TypeExpr {
            kind,
            span: Span {
                start,
                end: self.previous_span().end,
            },
        }
    }

    fn parse_expr(&mut self) -> Expr {
        self.parse_pipeline()
    }

    fn parse_pipeline(&mut self) -> Expr {
        let mut expr = self.parse_equality();
        while self.consume(TokenKind::PipeForward) {
            let rhs = self.parse_postfix();
            expr = self.pipe_expr(expr, rhs);
        }
        expr
    }

    fn pipe_expr(&mut self, input: Expr, stage: Expr) -> Expr {
        let start = input.span().start;
        let end = stage.span().end;
        match stage {
            Expr::Call {
                callee, mut args, ..
            } => {
                args.insert(0, input);
                Expr::Call {
                    callee,
                    args,
                    span: Span { start, end },
                }
            }
            other => Expr::Call {
                callee: Box::new(other),
                args: vec![input],
                span: Span { start, end },
            },
        }
    }

    fn parse_equality(&mut self) -> Expr {
        let mut expr = self.parse_comparison();
        loop {
            let op = match self.current_kind() {
                Some(TokenKind::EqEq) => BinaryOp::Eq,
                Some(TokenKind::NotEq) => BinaryOp::Ne,
                _ => break,
            };
            self.bump();
            let right = self.parse_comparison();
            let span = expr.span().join(right.span());
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(right),
                span,
            };
        }
        expr
    }

    fn parse_comparison(&mut self) -> Expr {
        let mut expr = self.parse_term();
        loop {
            let op = match self.current_kind() {
                Some(TokenKind::Less) => BinaryOp::Lt,
                Some(TokenKind::LessEq) => BinaryOp::Le,
                Some(TokenKind::Greater) => BinaryOp::Gt,
                Some(TokenKind::GreaterEq) => BinaryOp::Ge,
                _ => break,
            };
            self.bump();
            let right = self.parse_term();
            let span = expr.span().join(right.span());
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(right),
                span,
            };
        }
        expr
    }

    fn parse_term(&mut self) -> Expr {
        let mut expr = self.parse_factor();
        loop {
            let op = match self.current_kind() {
                Some(TokenKind::Plus) => BinaryOp::Add,
                Some(TokenKind::Minus) => BinaryOp::Sub,
                _ => break,
            };
            self.bump();
            let right = self.parse_factor();
            let span = expr.span().join(right.span());
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(right),
                span,
            };
        }
        expr
    }

    fn parse_factor(&mut self) -> Expr {
        let mut expr = self.parse_unary();
        loop {
            let op = match self.current_kind() {
                Some(TokenKind::Star) => BinaryOp::Mul,
                Some(TokenKind::Slash) => BinaryOp::Div,
                Some(TokenKind::Percent) => BinaryOp::Mod,
                _ => break,
            };
            self.bump();
            let right = self.parse_unary();
            let span = expr.span().join(right.span());
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(right),
                span,
            };
        }
        expr
    }

    fn parse_unary(&mut self) -> Expr {
        let op = match self.current_kind() {
            Some(TokenKind::Minus) => Some(UnaryOp::Neg),
            Some(TokenKind::KwAwait) => Some(UnaryOp::Await),
            Some(TokenKind::KwSpawn) => Some(UnaryOp::Spawn),
            Some(TokenKind::KwParallel) => Some(UnaryOp::Parallel),
            Some(TokenKind::KwMove) => Some(UnaryOp::Move),
            _ => None,
        };
        if let Some(op) = op {
            let start = self.bump().span.start;
            let expr = self.parse_unary();
            let span = Span {
                start,
                end: expr.span().end,
            };
            return Expr::Unary {
                op,
                expr: Box::new(expr),
                span,
            };
        }
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Expr {
        let mut expr = self.parse_primary();
        loop {
            match self.current_kind() {
                Some(TokenKind::LParen) => {
                    let start = expr.span().start;
                    self.bump();
                    let mut args = Vec::new();
                    while !self.check(TokenKind::RParen) && !self.is_eof() {
                        args.push(self.parse_expr());
                        if !self.consume(TokenKind::Comma) {
                            break;
                        }
                    }
                    let end = self.expect(TokenKind::RParen).end;
                    expr = Expr::Call {
                        callee: Box::new(expr),
                        args,
                        span: Span { start, end },
                    };
                }
                Some(TokenKind::Dot) => {
                    let start = expr.span().start;
                    self.bump();
                    let field = self.expect_ident();
                    expr = Expr::Member {
                        object: Box::new(expr),
                        field,
                        span: Span {
                            start,
                            end: self.previous_span().end,
                        },
                    };
                }
                Some(TokenKind::LBracket) => {
                    let start = expr.span().start;
                    self.bump();
                    let index = self.parse_expr();
                    let end = self.expect(TokenKind::RBracket).end;
                    expr = Expr::Index {
                        object: Box::new(expr),
                        index: Box::new(index),
                        span: Span { start, end },
                    };
                }
                _ => break,
            }
        }
        expr
    }

    fn parse_primary(&mut self) -> Expr {
        match self.bump_kind() {
            TokenKind::Int(value) => Expr::Int(value, self.previous_span()),
            TokenKind::Float(value) => Expr::Float(value, self.previous_span()),
            TokenKind::KwTrue => Expr::Bool(true, self.previous_span()),
            TokenKind::KwFalse => Expr::Bool(false, self.previous_span()),
            TokenKind::String(value) => Expr::String(value, self.previous_span()),
            TokenKind::Ident(name) => Expr::Ident(name, self.previous_span()),
            TokenKind::LParen => {
                let expr = self.parse_expr();
                self.expect(TokenKind::RParen);
                expr
            }
            TokenKind::LBracket => {
                let start = self.previous_span().start;
                let mut items = Vec::new();
                while !self.check(TokenKind::RBracket) && !self.is_eof() {
                    items.push(self.parse_expr());
                    if !self.consume(TokenKind::Comma) {
                        break;
                    }
                }
                let end = self.expect(TokenKind::RBracket).end;
                Expr::Array(items, Span { start, end })
            }
            other => {
                self.diagnostics.push(Diagnostic::error(
                    format!("unexpected token in expression: {:?}", other),
                    Some(self.previous_span()),
                ));
                Expr::Int(0, self.previous_span())
            }
        }
    }

    fn current_kind(&self) -> Option<&TokenKind> {
        self.tokens.get(self.pos).map(|t| &t.kind)
    }

    fn current_ident_is(&self, expected: &str) -> bool {
        matches!(self.current_kind(), Some(TokenKind::Ident(name)) if name == expected)
    }

    fn previous_kind(&self) -> Option<&TokenKind> {
        self.tokens.get(self.pos.saturating_sub(1)).map(|t| &t.kind)
    }

    fn peek_is(&self, kind: TokenKind) -> bool {
        self.tokens
            .get(self.pos + 1)
            .map(|t| t.kind == kind)
            .unwrap_or(false)
    }

    fn peek_kind(&self) -> Option<&TokenKind> {
        self.tokens.get(self.pos + 1).map(|t| &t.kind)
    }

    fn current_span(&self) -> Span {
        self.tokens
            .get(self.pos)
            .map(|t| t.span)
            .unwrap_or_default()
    }

    fn previous_span(&self) -> Span {
        self.tokens
            .get(self.pos.saturating_sub(1))
            .map(|t| t.span)
            .unwrap_or_default()
    }

    fn bump(&mut self) -> Token {
        let token = self.tokens.get(self.pos).cloned().unwrap_or(Token {
            kind: TokenKind::Eof,
            span: Span::default(),
        });
        self.pos = self.pos.saturating_add(1);
        token
    }

    fn bump_kind(&mut self) -> TokenKind {
        self.bump().kind
    }

    fn expect(&mut self, kind: TokenKind) -> Span {
        if self.check(kind.clone()) {
            self.bump().span
        } else {
            let span = self.current_span();
            self.diagnostics.push(Diagnostic::error(
                format!("expected {:?}, found {:?}", kind, self.current_kind()),
                Some(span),
            ));
            span
        }
    }

    fn expect_ident(&mut self) -> String {
        match self.bump_kind() {
            TokenKind::Ident(name) => name,
            other => {
                self.diagnostics.push(Diagnostic::error(
                    format!("expected identifier, found {:?}", other),
                    Some(self.previous_span()),
                ));
                "_error".to_string()
            }
        }
    }

    fn expect_alias_ident(&mut self) -> String {
        match self.bump_kind() {
            TokenKind::Ident(name) => name,
            TokenKind::KwAi => "ai".to_string(),
            TokenKind::KwActor => "actor".to_string(),
            TokenKind::KwAsync => "async".to_string(),
            other => {
                self.diagnostics.push(Diagnostic::error(
                    format!("expected import alias, found {:?}", other),
                    Some(self.previous_span()),
                ));
                "_error".to_string()
            }
        }
    }

    fn expect_named_ident(&mut self, expected: &str) {
        match self.bump_kind() {
            TokenKind::Ident(name) if name == expected => {}
            other => {
                self.diagnostics.push(Diagnostic::error(
                    format!("expected '{}', found {:?}", expected, other),
                    Some(self.previous_span()),
                ));
            }
        }
    }

    fn bump_path_segment(&mut self) -> Option<String> {
        let kind = self.bump_kind();
        match kind {
            TokenKind::Ident(name) => Some(name),
            TokenKind::KwAi => Some("ai".to_string()),
            TokenKind::KwAsync => Some("async".to_string()),
            TokenKind::KwActor => Some("actor".to_string()),
            TokenKind::KwFn => Some("fn".to_string()),
            TokenKind::KwState => Some("state".to_string()),
            TokenKind::KwLet => Some("let".to_string()),
            TokenKind::KwMatch => Some("match".to_string()),
            TokenKind::KwPrompt => Some("prompt".to_string()),
            _ => None,
        }
    }

    fn desugar_tail_return(
        &self,
        mut body: Vec<Stmt>,
        return_type: Option<&TypeExpr>,
    ) -> Vec<Stmt> {
        let Some(return_type) = return_type else {
            return body;
        };
        if matches!(
            &return_type.kind,
            TypeExprKind::Path(path) if path.len() == 1 && path[0] == "Unit"
        ) {
            return body;
        }
        if let Some(Stmt::Expr(expr)) = body.pop() {
            let span = expr.span();
            body.push(Stmt::Return {
                expr: Some(expr),
                span,
            });
            body
        } else {
            body
        }
    }

    fn consume(&mut self, kind: TokenKind) -> bool {
        if self.check(kind) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn consume_newline(&mut self) -> Option<Span> {
        if self.check(TokenKind::Newline) {
            Some(self.bump().span)
        } else {
            None
        }
    }

    fn check(&self, kind: TokenKind) -> bool {
        self.current_kind().map(|k| *k == kind).unwrap_or(false)
    }

    fn skip_newlines(&mut self) {
        while self.check(TokenKind::Newline) {
            self.bump();
        }
    }

    fn is_eof(&self) -> bool {
        matches!(self.current_kind(), Some(TokenKind::Eof) | None)
    }

    fn error_here(&mut self, message: impl Into<String>) {
        self.diagnostics
            .push(Diagnostic::error(message.into(), Some(self.current_span())));
    }
}
