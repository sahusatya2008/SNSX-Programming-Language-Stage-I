use crate::ast::{
    ActorDecl, ActorHandler, BinaryOp, Block, CapabilityDecl, EffectDecl, Expr, FieldDecl,
    FunctionDecl, MatchArm, ModuleDecl, Param, Pattern, Program, Stmt, TypeDecl, TypeExpr, UnaryOp,
    UseDecl, VariantDecl,
};
use crate::diag::Diagnostic;
use crate::lexer::{lex, Token, TokenKind};

pub fn parse(source: &str) -> Result<Program, Vec<Diagnostic>> {
    let tokens = lex(source)?;
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program();
    if parser.diagnostics.is_empty() {
        Ok(program)
    } else {
        Err(parser.diagnostics)
    }
}

struct Parser {
    tokens: Vec<Token>,
    index: usize,
    diagnostics: Vec<Diagnostic>,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            index: 0,
            diagnostics: Vec::new(),
        }
    }

    fn parse_program(&mut self) -> Program {
        let mut program = Program::new();
        while !self.at(TokenKind::Eof) {
            if self.at(TokenKind::Module) {
                if program.module.is_some() {
                    self.diagnostics.push(
                        Diagnostic::error("AERIS-PARSE-001", "module already declared")
                            .with_span(self.current().span.clone())
                            .with_help("AERIS programs may declare at most one module"),
                    );
                }
                program.module = Some(self.parse_module_decl());
            } else if self.at(TokenKind::Use) {
                program.uses.push(self.parse_use_decl());
            } else if self.at(TokenKind::Effect) {
                program.effects.push(self.parse_effect_decl());
            } else if self.at(TokenKind::Cap) {
                program.capabilities.push(self.parse_cap_decl());
            } else if self.at(TokenKind::Type) {
                program.types.push(self.parse_type_decl());
            } else if self.at(TokenKind::Fn) {
                program.functions.push(self.parse_function_decl());
            } else if self.at(TokenKind::Actor) {
                program.actors.push(self.parse_actor_decl());
            } else {
                let token = self.current().clone();
                self.diagnostics.push(
                    Diagnostic::error("AERIS-PARSE-002", "expected a top-level declaration")
                        .with_span(token.span)
                        .with_help("start with module/use/effect/cap/type/fn/actor"),
                );
                self.advance();
            }
        }
        program
    }

    fn parse_module_decl(&mut self) -> ModuleDecl {
        let start = self.expect(TokenKind::Module).span;
        let path = self.parse_path();
        self.expect(TokenKind::Semicolon);
        ModuleDecl { path, span: start }
    }

    fn parse_use_decl(&mut self) -> UseDecl {
        let start = self.expect(TokenKind::Use).span;
        let path = self.parse_path();
        let alias = if self.consume(TokenKind::As) {
            Some(self.expect_ident())
        } else {
            None
        };
        self.expect(TokenKind::Semicolon);
        UseDecl {
            path,
            alias,
            span: start,
        }
    }

    fn parse_effect_decl(&mut self) -> EffectDecl {
        let start = self.expect(TokenKind::Effect).span;
        let name = self.expect_ident();
        self.expect(TokenKind::Semicolon);
        EffectDecl { name, span: start }
    }

    fn parse_cap_decl(&mut self) -> CapabilityDecl {
        let start = self.expect(TokenKind::Cap).span;
        let name = self.expect_ident();
        self.expect(TokenKind::Semicolon);
        CapabilityDecl { name, span: start }
    }

    fn parse_type_decl(&mut self) -> TypeDecl {
        let start = self.expect(TokenKind::Type).span;
        let name = self.expect_ident();
        let generics = self.parse_generic_params();
        self.expect(TokenKind::Equal);
        let mut variants = Vec::new();
        loop {
            variants.push(self.parse_variant_decl());
            if !self.consume(TokenKind::Pipe) {
                break;
            }
        }
        self.expect(TokenKind::Semicolon);
        TypeDecl {
            name,
            generics,
            variants,
            span: start,
        }
    }

    fn parse_variant_decl(&mut self) -> VariantDecl {
        let span = self.current().span.clone();
        let name = self.expect_ident();
        let mut fields = Vec::new();
        if self.consume(TokenKind::LParen) {
            if !self.at(TokenKind::RParen) {
                loop {
                    let field_span = self.current().span.clone();
                    let field_name = self.expect_ident();
                    self.expect(TokenKind::Colon);
                    let ty = self.parse_type_expr();
                    fields.push(FieldDecl {
                        name: field_name,
                        ty,
                        span: field_span,
                    });
                    if !self.consume(TokenKind::Comma) {
                        break;
                    }
                }
            }
            self.expect(TokenKind::RParen);
        }
        VariantDecl { name, fields, span }
    }

    fn parse_function_decl(&mut self) -> FunctionDecl {
        let start = self.expect(TokenKind::Fn).span;
        let name = self.expect_ident();
        let generics = self.parse_generic_params();
        self.expect(TokenKind::LParen);
        let mut params = Vec::new();
        if !self.at(TokenKind::RParen) {
            loop {
                params.push(self.parse_param());
                if !self.consume(TokenKind::Comma) {
                    break;
                }
            }
        }
        self.expect(TokenKind::RParen);
        self.expect(TokenKind::Arrow);
        let return_type = self.parse_type_expr();
        let effect = if self.consume(TokenKind::Bang) {
            self.expect_ident()
        } else {
            self.diagnostics.push(
                Diagnostic::error("AERIS-PARSE-003", "functions must declare an effect")
                    .with_span(self.current().span.clone())
                    .with_help("append !pure, !io, !state, or !actor after the return type"),
            );
            "pure".to_string()
        };
        let where_clause = if self.consume(TokenKind::Where) {
            Some(self.parse_expr())
        } else {
            None
        };
        let body = self.parse_block();
        FunctionDecl {
            name,
            generics,
            params,
            return_type,
            effect,
            where_clause,
            body,
            span: start,
        }
    }

    fn parse_actor_decl(&mut self) -> ActorDecl {
        let start = self.expect(TokenKind::Actor).span;
        let name = self.expect_ident();
        self.expect(TokenKind::LParen);
        let mut params = Vec::new();
        if !self.at(TokenKind::RParen) {
            loop {
                params.push(self.parse_param());
                if !self.consume(TokenKind::Comma) {
                    break;
                }
            }
        }
        self.expect(TokenKind::RParen);
        self.expect(TokenKind::LBrace);
        let mut handlers = Vec::new();
        while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
            let span = self.expect(TokenKind::On).span;
            let pattern = self.parse_pattern();
            self.expect(TokenKind::FatArrow);
            let body = self.parse_block();
            handlers.push(ActorHandler {
                pattern,
                body,
                span,
            });
        }
        self.expect(TokenKind::RBrace);
        ActorDecl {
            name,
            params,
            handlers,
            span: start,
        }
    }

    fn parse_param(&mut self) -> Param {
        let span = self.current().span.clone();
        let name = self.expect_ident();
        self.expect(TokenKind::Colon);
        let ty = self.parse_type_expr();
        let refinement = if self.consume(TokenKind::Where) {
            Some(self.parse_expr())
        } else {
            None
        };
        Param {
            name,
            ty,
            refinement,
            span,
        }
    }

    fn parse_block(&mut self) -> Block {
        let start = self.expect(TokenKind::LBrace).span;
        let mut statements = Vec::new();
        let mut tail = None;
        while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
            if self.at(TokenKind::Let) {
                let span = self.expect(TokenKind::Let).span;
                let name = self.expect_ident();
                let value = if self.consume(TokenKind::Equal) {
                    Some(self.parse_expr())
                } else {
                    None
                };
                self.expect(TokenKind::Semicolon);
                statements.push(Stmt::Let { name, value, span });
            } else if self.at(TokenKind::Mut) {
                let span = self.expect(TokenKind::Mut).span;
                let name = self.expect_ident();
                self.expect(TokenKind::Equal);
                let value = self.parse_expr();
                self.expect(TokenKind::Semicolon);
                statements.push(Stmt::Mut { name, value, span });
            } else if self.at(TokenKind::Require) {
                let span = self.expect(TokenKind::Require).span;
                let condition = self.parse_expr();
                let message = if self.consume(TokenKind::Comma) {
                    match self.advance().kind {
                        TokenKind::Text(text) => Some(text),
                        _ => {
                            self.diagnostics.push(
                                Diagnostic::error(
                                    "AERIS-PARSE-004",
                                    "require message must be a string literal",
                                )
                                .with_span(self.current().span.clone()),
                            );
                            None
                        }
                    }
                } else {
                    None
                };
                self.expect(TokenKind::Semicolon);
                statements.push(Stmt::Require {
                    condition,
                    message,
                    span,
                });
            } else if self.at(TokenKind::Return) {
                let span = self.expect(TokenKind::Return).span;
                let value = self.parse_expr();
                self.expect(TokenKind::Semicolon);
                statements.push(Stmt::Return { value, span });
            } else {
                let expr = self.parse_expr();
                if self.consume(TokenKind::Semicolon) {
                    let span = self.previous().span.clone();
                    statements.push(Stmt::Expr { value: expr, span });
                } else {
                    tail = Some(Box::new(expr));
                    break;
                }
            }
        }
        self.expect(TokenKind::RBrace);
        Block {
            statements,
            tail,
            span: start,
        }
    }

    fn parse_expr(&mut self) -> Expr {
        self.parse_lambda()
    }

    fn parse_lambda(&mut self) -> Expr {
        if self.consume(TokenKind::Pipe) {
            let mut params = Vec::new();
            if !self.at(TokenKind::Pipe) {
                loop {
                    params.push(self.expect_ident());
                    if !self.consume(TokenKind::Comma) {
                        break;
                    }
                }
            }
            self.expect(TokenKind::Pipe);
            let body = self.parse_expr();
            Expr::Lambda {
                params,
                body: Box::new(body),
            }
        } else {
            self.parse_match_expr()
        }
    }

    fn parse_match_expr(&mut self) -> Expr {
        if self.consume(TokenKind::Match) {
            let scrutinee = self.parse_expr();
            self.expect(TokenKind::LBrace);
            let mut arms = Vec::new();
            while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
                let span = self.current().span.clone();
                let pattern = self.parse_pattern();
                self.expect(TokenKind::FatArrow);
                let body = self.parse_block();
                let _ = self.consume(TokenKind::Comma);
                arms.push(MatchArm {
                    pattern,
                    body,
                    span,
                });
            }
            self.expect(TokenKind::RBrace);
            Expr::Match {
                scrutinee: Box::new(scrutinee),
                arms,
            }
        } else {
            self.parse_if_expr()
        }
    }

    fn parse_if_expr(&mut self) -> Expr {
        if self.consume(TokenKind::If) {
            let condition = self.parse_expr();
            let then_branch = self.parse_block();
            let else_branch = if self.consume(TokenKind::Else) {
                self.parse_block()
            } else {
                Block {
                    statements: Vec::new(),
                    tail: Some(Box::new(Expr::Bool(false))),
                    span: then_branch.span.clone(),
                }
            };
            Expr::If {
                condition: Box::new(condition),
                then_branch: Box::new(then_branch),
                else_branch: Box::new(else_branch),
            }
        } else {
            self.parse_or()
        }
    }

    fn parse_or(&mut self) -> Expr {
        let mut expr = self.parse_and();
        while self.consume(TokenKind::PipePipe) {
            let right = self.parse_and();
            expr = Expr::Binary {
                op: BinaryOp::Or,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        expr
    }

    fn parse_and(&mut self) -> Expr {
        let mut expr = self.parse_equality();
        while self.consume(TokenKind::AmpAmp) {
            let right = self.parse_equality();
            expr = Expr::Binary {
                op: BinaryOp::And,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        expr
    }

    fn parse_equality(&mut self) -> Expr {
        let mut expr = self.parse_comparison();
        loop {
            let op = if self.consume(TokenKind::EqEq) {
                Some(BinaryOp::Eq)
            } else if self.consume(TokenKind::NotEq) {
                Some(BinaryOp::Ne)
            } else {
                None
            };
            if let Some(op) = op {
                let right = self.parse_comparison();
                expr = Expr::Binary {
                    op,
                    left: Box::new(expr),
                    right: Box::new(right),
                };
            } else {
                break;
            }
        }
        expr
    }

    fn parse_comparison(&mut self) -> Expr {
        let mut expr = self.parse_term();
        loop {
            let op = if self.consume(TokenKind::Less) {
                Some(BinaryOp::Lt)
            } else if self.consume(TokenKind::LessEq) {
                Some(BinaryOp::Le)
            } else if self.consume(TokenKind::Greater) {
                Some(BinaryOp::Gt)
            } else if self.consume(TokenKind::GreaterEq) {
                Some(BinaryOp::Ge)
            } else {
                None
            };
            if let Some(op) = op {
                let right = self.parse_term();
                expr = Expr::Binary {
                    op,
                    left: Box::new(expr),
                    right: Box::new(right),
                };
            } else {
                break;
            }
        }
        expr
    }

    fn parse_term(&mut self) -> Expr {
        let mut expr = self.parse_factor();
        loop {
            let op = if self.consume(TokenKind::Plus) {
                Some(BinaryOp::Add)
            } else if self.consume(TokenKind::Minus) {
                Some(BinaryOp::Sub)
            } else {
                None
            };
            if let Some(op) = op {
                let right = self.parse_factor();
                expr = Expr::Binary {
                    op,
                    left: Box::new(expr),
                    right: Box::new(right),
                };
            } else {
                break;
            }
        }
        expr
    }

    fn parse_factor(&mut self) -> Expr {
        let mut expr = self.parse_unary();
        loop {
            let op = if self.consume(TokenKind::Star) {
                Some(BinaryOp::Mul)
            } else if self.consume(TokenKind::Slash) {
                Some(BinaryOp::Div)
            } else if self.consume(TokenKind::Percent) {
                Some(BinaryOp::Mod)
            } else {
                None
            };
            if let Some(op) = op {
                let right = self.parse_unary();
                expr = Expr::Binary {
                    op,
                    left: Box::new(expr),
                    right: Box::new(right),
                };
            } else {
                break;
            }
        }
        expr
    }

    fn parse_unary(&mut self) -> Expr {
        if self.consume(TokenKind::Minus) {
            let value = self.parse_unary();
            Expr::Unary {
                op: UnaryOp::Neg,
                value: Box::new(value),
            }
        } else if self.consume(TokenKind::Bang) {
            let value = self.parse_unary();
            Expr::Unary {
                op: UnaryOp::Not,
                value: Box::new(value),
            }
        } else {
            self.parse_call()
        }
    }

    fn parse_call(&mut self) -> Expr {
        let mut expr = self.parse_primary();
        while self.consume(TokenKind::LParen) {
            let mut args = Vec::new();
            if !self.at(TokenKind::RParen) {
                loop {
                    args.push(self.parse_expr());
                    if !self.consume(TokenKind::Comma) {
                        break;
                    }
                }
            }
            self.expect(TokenKind::RParen);
            expr = Expr::Call {
                callee: Box::new(expr),
                args,
            };
        }
        expr
    }

    fn parse_primary(&mut self) -> Expr {
        let token = self.advance();
        match token.kind {
            TokenKind::Int(value) => Expr::Int(value),
            TokenKind::True => Expr::Bool(true),
            TokenKind::False => Expr::Bool(false),
            TokenKind::Text(value) => Expr::Text(value),
            TokenKind::Ident(value) => Expr::Variable(value),
            TokenKind::LParen => {
                let expr = self.parse_expr();
                self.expect(TokenKind::RParen);
                expr
            }
            _ => {
                self.diagnostics.push(
                    Diagnostic::error("AERIS-PARSE-005", "expected an expression")
                        .with_span(token.span)
                        .with_help(
                            "valid expressions include literals, calls, variables, if, and match",
                        ),
                );
                Expr::Int(0)
            }
        }
    }

    fn parse_pattern(&mut self) -> Pattern {
        let token = self.advance();
        match token.kind {
            TokenKind::Ident(value) if value == "_" => Pattern::Wildcard,
            TokenKind::Ident(value) => {
                if self.consume(TokenKind::LParen) {
                    let mut bindings = Vec::new();
                    if !self.at(TokenKind::RParen) {
                        loop {
                            bindings.push(self.expect_ident());
                            if !self.consume(TokenKind::Comma) {
                                break;
                            }
                        }
                    }
                    self.expect(TokenKind::RParen);
                    Pattern::Variant {
                        name: value,
                        bindings,
                    }
                } else {
                    Pattern::Identifier(value)
                }
            }
            TokenKind::Int(value) => Pattern::Int(value),
            TokenKind::True => Pattern::Bool(true),
            TokenKind::False => Pattern::Bool(false),
            TokenKind::Text(value) => Pattern::Text(value),
            _ => {
                self.diagnostics.push(
                    Diagnostic::error("AERIS-PARSE-006", "invalid pattern")
                        .with_span(token.span)
                        .with_help("patterns may be literals, identifiers, variants, or '_'"),
                );
                Pattern::Wildcard
            }
        }
    }

    fn parse_type_expr(&mut self) -> TypeExpr {
        let name = self.expect_ident();
        let mut args = Vec::new();
        if self.consume(TokenKind::Less) {
            loop {
                args.push(self.parse_type_expr());
                if !self.consume(TokenKind::Comma) {
                    break;
                }
            }
            self.expect(TokenKind::Greater);
        }
        TypeExpr::Named { name, args }
    }

    fn parse_path(&mut self) -> Vec<String> {
        let mut path = vec![self.expect_ident()];
        while self.consume(TokenKind::Dot) {
            path.push(self.expect_ident());
        }
        path
    }

    fn parse_generic_params(&mut self) -> Vec<String> {
        let mut generics = Vec::new();
        if self.consume(TokenKind::Less) {
            loop {
                generics.push(self.expect_ident());
                if !self.consume(TokenKind::Comma) {
                    break;
                }
            }
            self.expect(TokenKind::Greater);
        }
        generics
    }

    fn expect_ident(&mut self) -> String {
        let token = self.advance();
        match token.kind {
            TokenKind::Ident(value) => value,
            TokenKind::Actor => "actor".to_string(),
            _ => {
                self.diagnostics.push(
                    Diagnostic::error("AERIS-PARSE-007", "expected an identifier")
                        .with_span(token.span),
                );
                "_error".to_string()
            }
        }
    }

    fn expect(&mut self, expected: TokenKind) -> Token {
        if self.at(expected.clone()) {
            self.advance()
        } else {
            let token = self.current().clone();
            self.diagnostics.push(
                Diagnostic::error("AERIS-PARSE-008", format!("expected {:?}", expected))
                    .with_span(token.span.clone()),
            );
            token
        }
    }

    fn consume(&mut self, expected: TokenKind) -> bool {
        if self.at(expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn at(&self, expected: TokenKind) -> bool {
        self.current().kind == expected
    }

    fn current(&self) -> &Token {
        &self.tokens[self.index]
    }

    fn previous(&self) -> &Token {
        &self.tokens[self.index.saturating_sub(1)]
    }

    fn advance(&mut self) -> Token {
        let token = self.tokens[self.index].clone();
        if !matches!(token.kind, TokenKind::Eof) {
            self.index += 1;
        }
        token
    }
}
