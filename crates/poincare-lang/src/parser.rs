//! Recursive-descent parser with precedence-climbing expressions.
//!
//! Implements the frozen V1 grammar in
//! `docs/plans/poincare-notebook/poincare-language-v1-spec.md`. On error it
//! records a diagnostic and synchronizes to the next statement separator so
//! multiple errors can be reported from one parse.

use crate::ast::*;
use crate::diagnostic::Diagnostic;
use crate::lexer::lex;
use crate::span::Span;
use crate::token::{Token, TokenKind};

/// The result of parsing: a (possibly partial) program plus diagnostics.
#[derive(Clone, Debug)]
pub struct ParseResult {
    pub program: Program,
    pub diagnostics: Vec<Diagnostic>,
}

impl ParseResult {
    pub fn has_errors(&self) -> bool {
        !self.diagnostics.is_empty()
    }
}

/// Parse Poincare source into a program and diagnostics.
pub fn parse(src: &str) -> ParseResult {
    let (tokens, mut diagnostics) = lex(src);
    let mut parser = Parser {
        tokens,
        pos: 0,
        diags: Vec::new(),
    };
    let program = parser.parse_program();
    diagnostics.append(&mut parser.diags);
    ParseResult {
        program,
        diagnostics,
    }
}

/// Recognized plot-kind words in `plot <kind> ...`.
const PLOT_KINDS: &[&str] = &[
    "surface",
    "curve",
    "scatter",
    "vector_field",
    "volume",
    "isosurface",
    "points",
    "line",
];

type PResult<T> = Result<T, ()>;

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    diags: Vec<Diagnostic>,
}

impl Parser {
    // --- token cursor ---

    fn kind(&self) -> &TokenKind {
        &self.tokens[self.pos].kind
    }

    fn nth_kind(&self, k: usize) -> &TokenKind {
        let i = (self.pos + k).min(self.tokens.len() - 1);
        &self.tokens[i].kind
    }

    fn span(&self) -> Span {
        self.tokens[self.pos].span
    }

    fn prev_span(&self) -> Span {
        self.tokens[self.pos.saturating_sub(1)].span
    }

    fn at(&self, kind: &TokenKind) -> bool {
        self.kind() == kind
    }

    fn at_eof(&self) -> bool {
        matches!(self.kind(), TokenKind::Eof)
    }

    fn advance(&mut self) -> Token {
        let tok = self.tokens[self.pos].clone();
        if !self.at_eof() {
            self.pos += 1;
        }
        tok
    }

    fn eat(&mut self, kind: &TokenKind) -> bool {
        if self.at(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, kind: &TokenKind, ctx: &str) -> PResult<Token> {
        if self.at(kind) {
            Ok(self.advance())
        } else {
            self.error(format!(
                "expected {}{}, found {}",
                kind.describe(),
                ctx,
                self.kind().describe()
            ))
        }
    }

    fn error<T>(&mut self, message: impl Into<String>) -> PResult<T> {
        self.diags
            .push(Diagnostic::error(message.into(), self.span()));
        Err(())
    }

    // --- separator handling ---

    fn skip_newlines(&mut self) {
        while matches!(self.kind(), TokenKind::Newline) {
            self.advance();
        }
    }

    fn skip_separators(&mut self) {
        while matches!(self.kind(), TokenKind::Newline | TokenKind::Semicolon) {
            self.advance();
        }
    }

    fn at_separator_or_end(&self) -> bool {
        matches!(
            self.kind(),
            TokenKind::Newline | TokenKind::Semicolon | TokenKind::Eof | TokenKind::RBrace
        )
    }

    /// Skip tokens until the next statement separator, for error recovery.
    fn synchronize(&mut self) {
        while !self.at_eof() {
            if matches!(self.kind(), TokenKind::Newline | TokenKind::Semicolon) {
                self.advance();
                return;
            }
            if matches!(self.kind(), TokenKind::RBrace) {
                return;
            }
            self.advance();
        }
    }

    // --- program ---

    fn parse_program(&mut self) -> Program {
        let mut stmts = Vec::new();
        self.skip_separators();
        while !self.at_eof() {
            match self.parse_stmt() {
                Ok(stmt) => {
                    stmts.push(stmt);
                    if self.at_eof() {
                        break;
                    }
                    if !self.at_separator_or_end() {
                        let _ = self.error::<()>(format!(
                            "expected end of statement, found {}",
                            self.kind().describe()
                        ));
                        self.synchronize();
                    }
                    self.skip_separators();
                }
                Err(()) => {
                    self.synchronize();
                    self.skip_separators();
                }
            }
        }
        Program { stmts }
    }

    // --- statements ---

    fn parse_stmt(&mut self) -> PResult<Stmt> {
        match self.kind() {
            TokenKind::For => self.parse_for().map(Stmt::For),
            TokenKind::Plot => self.parse_plot().map(Stmt::Plot),
            TokenKind::Fn => self.parse_fn_def().map(Stmt::Func),
            TokenKind::Reserved(w) => {
                let w = *w;
                self.error(format!("`{w}` is reserved for a future language version"))
            }
            TokenKind::Ident(_) => match self.nth_kind(1) {
                TokenKind::Colon => self.parse_signature().map(Stmt::Signature),
                TokenKind::Eq => self.parse_binding().map(Stmt::Binding),
                TokenKind::LParen if self.is_func_def_ahead() => {
                    self.parse_expr_func_def().map(Stmt::Func)
                }
                _ => self.parse_expr().map(Stmt::Expr),
            },
            _ => self.parse_expr().map(Stmt::Expr),
        }
    }

    /// Lookahead: an identifier followed by a balanced `(...)` then `=` (and not
    /// `==`) is an expression-function definition; otherwise it is a call.
    fn is_func_def_ahead(&self) -> bool {
        // self.pos is the identifier; self.pos+1 is `(`.
        let mut i = self.pos + 1;
        let mut depth = 0i32;
        while i < self.tokens.len() {
            match &self.tokens[i].kind {
                TokenKind::LParen => depth += 1,
                TokenKind::RParen => {
                    depth -= 1;
                    if depth == 0 {
                        // Token after the matching `)`.
                        let next = self.tokens.get(i + 1).map(|t| &t.kind);
                        return matches!(next, Some(TokenKind::Eq));
                    }
                }
                TokenKind::Eof => return false,
                _ => {}
            }
            i += 1;
        }
        false
    }

    fn parse_signature(&mut self) -> PResult<Signature> {
        let name = self.parse_ident("in signature")?;
        self.expect(&TokenKind::Colon, " in signature")?;
        let mut types = vec![self.parse_expr()?];
        while self.eat(&TokenKind::Arrow) {
            self.skip_newlines();
            types.push(self.parse_expr()?);
        }
        let span = name.span.to(self.prev_span());
        Ok(Signature { name, types, span })
    }

    fn parse_binding(&mut self) -> PResult<Binding> {
        let name = self.parse_ident("in binding")?;
        self.expect(&TokenKind::Eq, " in binding")?;
        self.skip_newlines();
        let value = self.parse_expr()?;
        let span = name.span.to(value.span());
        Ok(Binding { name, value, span })
    }

    fn parse_expr_func_def(&mut self) -> PResult<FuncDef> {
        let name = self.parse_ident("in function definition")?;
        let params = self.parse_param_list()?;
        self.expect(&TokenKind::Eq, " in function definition")?;
        self.skip_newlines();
        let body = self.parse_expr()?;
        let span = name.span.to(body.span());
        Ok(FuncDef {
            name,
            params,
            body,
            kind: FuncKind::Expr,
            span,
        })
    }

    fn parse_fn_def(&mut self) -> PResult<FuncDef> {
        let start = self.span();
        self.expect(&TokenKind::Fn, "")?;
        let name = self.parse_ident("after `fn`")?;
        let params = self.parse_param_list()?;
        self.skip_newlines();
        let block = self.parse_block()?;
        let span = start.to(block.span);
        Ok(FuncDef {
            name,
            params,
            body: Expr::Block(block),
            kind: FuncKind::Block,
            span,
        })
    }

    fn parse_param_list(&mut self) -> PResult<Vec<Ident>> {
        self.expect(&TokenKind::LParen, " in parameter list")?;
        let mut params = Vec::new();
        self.skip_newlines();
        if !self.at(&TokenKind::RParen) {
            loop {
                params.push(self.parse_ident("in parameter list")?);
                self.skip_newlines();
                if self.eat(&TokenKind::Comma) {
                    self.skip_newlines();
                    continue;
                }
                break;
            }
        }
        self.expect(&TokenKind::RParen, " in parameter list")?;
        Ok(params)
    }

    fn parse_for(&mut self) -> PResult<ForStmt> {
        let start = self.span();
        self.expect(&TokenKind::For, "")?;
        self.skip_newlines();
        let var = self.parse_ident("after `for`")?;
        self.skip_newlines();
        self.expect(&TokenKind::In, " in `for` loop")?;
        self.skip_newlines();
        let iter = self.parse_expr()?;
        self.skip_newlines();
        let body = self.parse_block()?;
        let span = start.to(body.span);
        Ok(ForStmt {
            var,
            iter,
            body,
            span,
        })
    }

    fn parse_plot(&mut self) -> PResult<PlotStmt> {
        let start = self.span();
        self.expect(&TokenKind::Plot, "")?;

        // Optional plot kind.
        let kind = if let TokenKind::Ident(name) = self.kind() {
            if PLOT_KINDS.contains(&name.as_str()) {
                Some(self.parse_ident("plot kind")?)
            } else {
                None
            }
        } else {
            None
        };

        // Optional target expression (not a block, `over`, or a separator).
        let target = if matches!(
            self.kind(),
            TokenKind::LBrace | TokenKind::Over | TokenKind::Newline | TokenKind::Semicolon
        ) || self.at_eof()
        {
            None
        } else {
            Some(self.parse_expr()?)
        };

        // Optional `over` clause.
        let mut over = Vec::new();
        if self.eat(&TokenKind::Over) {
            self.skip_newlines();
            loop {
                over.push(self.parse_field()?);
                if self.eat(&TokenKind::Comma) {
                    self.skip_newlines();
                    continue;
                }
                break;
            }
        }

        // Optional config block (must be immediately present).
        let mut fields = Vec::new();
        if self.at(&TokenKind::LBrace) {
            self.advance();
            self.skip_separators();
            while !self.at(&TokenKind::RBrace) && !self.at_eof() {
                fields.push(self.parse_field()?);
                if !self.at(&TokenKind::RBrace) {
                    if !matches!(
                        self.kind(),
                        TokenKind::Newline | TokenKind::Semicolon | TokenKind::Comma
                    ) {
                        return self.error(format!(
                            "expected field separator in plot block, found {}",
                            self.kind().describe()
                        ));
                    }
                    self.skip_separators();
                    self.skip_commas();
                }
            }
            self.expect(&TokenKind::RBrace, " to close plot block")?;
        }

        if kind.is_none() && target.is_none() && over.is_empty() && fields.is_empty() {
            return self.error("`plot` needs a target, kind, or block");
        }

        let span = start.to(self.prev_span());
        Ok(PlotStmt {
            kind,
            target,
            over,
            fields,
            span,
        })
    }

    fn skip_commas(&mut self) {
        while matches!(
            self.kind(),
            TokenKind::Comma | TokenKind::Newline | TokenKind::Semicolon
        ) {
            self.advance();
        }
    }

    fn parse_field(&mut self) -> PResult<Field> {
        let name = self.parse_ident("in field")?;
        self.expect(&TokenKind::Eq, " in field")?;
        self.skip_newlines();
        let value = self.parse_expr()?;
        let span = name.span.to(value.span());
        Ok(Field { name, value, span })
    }

    fn parse_ident(&mut self, ctx: &str) -> PResult<Ident> {
        if let TokenKind::Ident(name) = self.kind() {
            let sym = Symbol::new(name.clone());
            let span = self.span();
            self.advance();
            Ok(Ident { sym, span })
        } else {
            self.error(format!(
                "expected identifier {ctx}, found {}",
                self.kind().describe()
            ))
        }
    }

    // --- blocks ---

    fn parse_block(&mut self) -> PResult<Block> {
        let start = self.span();
        self.expect(&TokenKind::LBrace, " to open block")?;
        let mut stmts = Vec::new();
        let mut tail = None;
        loop {
            self.skip_separators();
            if self.at(&TokenKind::RBrace) || self.at_eof() {
                break;
            }
            let stmt = self.parse_stmt()?;
            let had_sep = matches!(self.kind(), TokenKind::Newline | TokenKind::Semicolon);
            self.skip_separators();
            if self.at(&TokenKind::RBrace) || self.at_eof() {
                // Last item: an expression with no following statement is the
                // block's tail value.
                if let Stmt::Expr(e) = stmt {
                    tail = Some(Box::new(e));
                } else {
                    stmts.push(stmt);
                }
                break;
            }
            if !had_sep {
                return self.error(format!(
                    "expected end of statement in block, found {}",
                    self.kind().describe()
                ));
            }
            stmts.push(stmt);
        }
        let close = self.expect(&TokenKind::RBrace, " to close block")?;
        let span = start.to(close.span);
        Ok(Block { stmts, tail, span })
    }

    // --- expressions (precedence climbing) ---

    fn parse_expr(&mut self) -> PResult<Expr> {
        self.parse_expr_bp(0)
    }

    fn parse_expr_bp(&mut self, min_bp: u8) -> PResult<Expr> {
        let mut lhs = self.parse_prefix()?;

        loop {
            let Some(op) = InfixOp::from_kind(self.kind()) else {
                break;
            };
            if op.left_bp < min_bp {
                break;
            }
            self.advance();
            self.skip_newlines();
            let rhs = self.parse_expr_bp(op.right_bp)?;
            let span = lhs.span().to(rhs.span());
            lhs = op.build(lhs, rhs, span);

            // Non-associative operators (comparisons, range) may not chain.
            if op.non_assoc
                && let Some(next) = InfixOp::from_kind(self.kind())
                && next.class == op.class
            {
                return self.error("chained comparisons/ranges are not allowed; parenthesize");
            }
        }
        Ok(lhs)
    }

    fn parse_prefix(&mut self) -> PResult<Expr> {
        // Lambda: `x => body`.
        if matches!(self.kind(), TokenKind::Ident(_))
            && matches!(self.nth_kind(1), TokenKind::FatArrow)
        {
            let param = self.parse_ident("lambda parameter")?;
            let start = param.span;
            self.expect(&TokenKind::FatArrow, "")?;
            self.skip_newlines();
            let body = self.parse_expr_bp(0)?;
            let span = start.to(body.span());
            return Ok(Expr::Lambda {
                params: vec![param],
                body: Box::new(body),
                span,
            });
        }

        match self.kind() {
            TokenKind::Minus => {
                let start = self.span();
                self.advance();
                let expr = self.parse_expr_bp(UNARY_BP)?;
                let span = start.to(expr.span());
                Ok(Expr::Unary {
                    op: UnaryOp::Neg,
                    expr: Box::new(expr),
                    span,
                })
            }
            TokenKind::Not => {
                let start = self.span();
                self.advance();
                let expr = self.parse_expr_bp(UNARY_BP)?;
                let span = start.to(expr.span());
                Ok(Expr::Unary {
                    op: UnaryOp::Not,
                    expr: Box::new(expr),
                    span,
                })
            }
            _ => {
                let atom = self.parse_atom()?;
                self.parse_postfix(atom)
            }
        }
    }

    fn parse_postfix(&mut self, mut expr: Expr) -> PResult<Expr> {
        loop {
            match self.kind() {
                TokenKind::LParen => {
                    self.advance();
                    let args = self.parse_args()?;
                    let close = self.expect(&TokenKind::RParen, " to close call")?;
                    let span = expr.span().to(close.span);
                    expr = Expr::Call {
                        callee: Box::new(expr),
                        args,
                        span,
                    };
                }
                TokenKind::LBracket => {
                    self.advance();
                    self.skip_newlines();
                    let index = self.parse_expr()?;
                    self.skip_newlines();
                    let close = self.expect(&TokenKind::RBracket, " to close index")?;
                    let span = expr.span().to(close.span);
                    expr = Expr::Index {
                        base: Box::new(expr),
                        index: Box::new(index),
                        span,
                    };
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    fn parse_args(&mut self) -> PResult<Vec<Arg>> {
        let mut args = Vec::new();
        self.skip_newlines();
        if self.at(&TokenKind::RParen) {
            return Ok(args);
        }
        loop {
            self.skip_newlines();
            // Named argument: `name = value` (but not `==`).
            let arg = if matches!(self.kind(), TokenKind::Ident(_))
                && matches!(self.nth_kind(1), TokenKind::Eq)
            {
                let name = self.parse_ident("in argument")?;
                self.advance(); // `=`
                self.skip_newlines();
                let value = self.parse_expr()?;
                let span = name.span.to(value.span());
                Arg {
                    name: Some(name),
                    value,
                    span,
                }
            } else {
                let value = self.parse_expr()?;
                let span = value.span();
                Arg {
                    name: None,
                    value,
                    span,
                }
            };
            args.push(arg);
            self.skip_newlines();
            if self.eat(&TokenKind::Comma) {
                self.skip_newlines();
                if self.at(&TokenKind::RParen) {
                    break;
                }
                continue;
            }
            break;
        }
        Ok(args)
    }

    fn parse_atom(&mut self) -> PResult<Expr> {
        let span = self.span();
        match self.kind().clone() {
            TokenKind::Int(raw) => {
                self.advance();
                Ok(Expr::Int { raw, span })
            }
            TokenKind::Float(raw) => {
                self.advance();
                Ok(Expr::Float { raw, span })
            }
            TokenKind::Str(value) => {
                self.advance();
                Ok(Expr::Str { value, span })
            }
            TokenKind::True => {
                self.advance();
                Ok(Expr::Bool { value: true, span })
            }
            TokenKind::False => {
                self.advance();
                Ok(Expr::Bool { value: false, span })
            }
            TokenKind::Ident(name) => {
                self.advance();
                Ok(Expr::Ident(Ident {
                    sym: Symbol::new(name),
                    span,
                }))
            }
            TokenKind::LParen => self.parse_paren_or_lambda(),
            TokenKind::LBracket => self.parse_list(),
            TokenKind::LBrace => Ok(Expr::Block(self.parse_block()?)),
            TokenKind::If => Ok(Expr::If(self.parse_if()?)),
            TokenKind::Reserved(w) => {
                self.error(format!("`{w}` is reserved for a future language version"))
            }
            other => self.error(format!("expected expression, found {}", other.describe())),
        }
    }

    fn parse_paren_or_lambda(&mut self) -> PResult<Expr> {
        let start = self.span();
        self.expect(&TokenKind::LParen, "")?;
        self.skip_newlines();

        // Empty parens: only valid as a zero-arg lambda `() => e`.
        if self.at(&TokenKind::RParen) {
            self.advance();
            if self.at(&TokenKind::FatArrow) {
                self.advance();
                self.skip_newlines();
                let body = self.parse_expr_bp(0)?;
                let span = start.to(body.span());
                return Ok(Expr::Lambda {
                    params: Vec::new(),
                    body: Box::new(body),
                    span,
                });
            }
            return self.error("empty parentheses are not a valid expression");
        }

        let mut items = vec![self.parse_expr()?];
        self.skip_newlines();
        while self.eat(&TokenKind::Comma) {
            self.skip_newlines();
            items.push(self.parse_expr()?);
            self.skip_newlines();
        }
        let close = self.expect(&TokenKind::RParen, " to close parentheses")?;

        if self.at(&TokenKind::FatArrow) {
            // Lambda parameter list: every item must be a bare identifier.
            self.advance();
            let mut params = Vec::new();
            for item in items {
                match item {
                    Expr::Ident(id) => params.push(id),
                    other => {
                        return {
                            self.diags.push(Diagnostic::error(
                                "lambda parameters must be identifiers",
                                other.span(),
                            ));
                            Err(())
                        };
                    }
                }
            }
            self.skip_newlines();
            let body = self.parse_expr_bp(0)?;
            let span = start.to(body.span());
            return Ok(Expr::Lambda {
                params,
                body: Box::new(body),
                span,
            });
        }

        if items.len() == 1 {
            Ok(items.into_iter().next().unwrap())
        } else {
            self.diags.push(Diagnostic::error(
                "expected `=>` after parameter list (tuples are not supported)",
                start.to(close.span),
            ));
            Err(())
        }
    }

    fn parse_list(&mut self) -> PResult<Expr> {
        let start = self.span();
        self.expect(&TokenKind::LBracket, "")?;
        let mut items = Vec::new();
        self.skip_newlines();
        if !self.at(&TokenKind::RBracket) {
            loop {
                items.push(self.parse_expr()?);
                self.skip_newlines();
                if self.eat(&TokenKind::Comma) {
                    self.skip_newlines();
                    if self.at(&TokenKind::RBracket) {
                        break;
                    }
                    continue;
                }
                break;
            }
        }
        let close = self.expect(&TokenKind::RBracket, " to close list")?;
        let span = start.to(close.span);
        Ok(Expr::List { items, span })
    }

    fn parse_if(&mut self) -> PResult<IfExpr> {
        let start = self.span();
        self.expect(&TokenKind::If, "")?;
        self.skip_newlines();
        let cond = self.parse_expr()?;
        self.skip_newlines();
        let then_block = self.parse_block()?;

        // `else` may follow across a newline.
        let mut els = None;
        let save = self.pos;
        self.skip_newlines();
        if self.eat(&TokenKind::Else) {
            self.skip_newlines();
            if self.at(&TokenKind::If) {
                els = Some(Box::new(ElseBranch::If(self.parse_if()?)));
            } else {
                els = Some(Box::new(ElseBranch::Block(self.parse_block()?)));
            }
        } else {
            // No else: restore the separator we skipped so it terminates the
            // statement.
            self.pos = save;
        }

        let span = start.to(self.prev_span());
        Ok(IfExpr {
            cond: Box::new(cond),
            then_block,
            els,
            span,
        })
    }
}

/// Binding power for the operand of a prefix unary operator. Sits above the
/// composition/multiplicative operators but below `^`, so `-x^2` is `-(x^2)`.
const UNARY_BP: u8 = 17;

#[derive(Clone, Copy, PartialEq, Eq)]
enum OpClass {
    Pipe,
    Or,
    And,
    Cmp,
    Range,
    Additive,
    Multiplicative,
    Compose,
    Pow,
}

struct InfixOp {
    left_bp: u8,
    right_bp: u8,
    non_assoc: bool,
    class: OpClass,
    build_kind: BuildKind,
}

enum BuildKind {
    Binary(BinaryOp),
    Compose,
    Pipe,
    Range,
}

impl InfixOp {
    fn from_kind(kind: &TokenKind) -> Option<InfixOp> {
        use BinaryOp as B;
        use BuildKind as K;
        use OpClass as C;
        let (left_bp, right_bp, non_assoc, class, build_kind) = match kind {
            TokenKind::Pipe => (1, 2, false, C::Pipe, K::Pipe),
            TokenKind::Or => (3, 4, false, C::Or, K::Binary(B::Or)),
            TokenKind::And => (5, 6, false, C::And, K::Binary(B::And)),
            TokenKind::EqEq => (7, 8, true, C::Cmp, K::Binary(B::Eq)),
            TokenKind::NotEq => (7, 8, true, C::Cmp, K::Binary(B::Ne)),
            TokenKind::Lt => (7, 8, true, C::Cmp, K::Binary(B::Lt)),
            TokenKind::Le => (7, 8, true, C::Cmp, K::Binary(B::Le)),
            TokenKind::Gt => (7, 8, true, C::Cmp, K::Binary(B::Gt)),
            TokenKind::Ge => (7, 8, true, C::Cmp, K::Binary(B::Ge)),
            TokenKind::DotDot => (9, 10, true, C::Range, K::Range),
            TokenKind::Plus => (11, 12, false, C::Additive, K::Binary(B::Add)),
            TokenKind::Minus => (11, 12, false, C::Additive, K::Binary(B::Sub)),
            TokenKind::Star => (13, 14, false, C::Multiplicative, K::Binary(B::Mul)),
            TokenKind::Slash => (13, 14, false, C::Multiplicative, K::Binary(B::Div)),
            TokenKind::Percent => (13, 14, false, C::Multiplicative, K::Binary(B::Rem)),
            // Composition is right-associative: left_bp > right_bp.
            TokenKind::Dot => (16, 15, false, C::Compose, K::Compose),
            // Power is right-associative and binds tightest among infix ops.
            TokenKind::Caret => (20, 19, false, C::Pow, K::Binary(B::Pow)),
            _ => return None,
        };
        Some(InfixOp {
            left_bp,
            right_bp,
            non_assoc,
            class,
            build_kind,
        })
    }

    fn build(&self, lhs: Expr, rhs: Expr, span: Span) -> Expr {
        match self.build_kind {
            BuildKind::Binary(op) => Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            },
            BuildKind::Compose => Expr::Compose {
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            },
            BuildKind::Pipe => Expr::Pipe {
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            },
            BuildKind::Range => Expr::Range {
                lo: Box::new(lhs),
                hi: Box::new(rhs),
                span,
            },
        }
    }
}
