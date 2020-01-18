use crate::error::{ParseErrKind, RubyError};
use crate::lexer::Lexer;
use crate::node::*;
use crate::token::*;
use crate::util::*;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct Parser {
    pub lexer: Lexer,
    tokens: Vec<Token>,
    cursor: usize,
    prev_cursor: usize,
    context_stack: Vec<Context>,
    pub ident_table: IdentifierTable,
    state_save: Vec<(usize, usize)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParseResult {
    pub node: Node,
    pub ident_table: IdentifierTable,
    pub lvar_collector: LvarCollector,
    pub source_info: SourceInfoRef,
}

impl ParseResult {
    pub fn default(node: Node, lvar_collector: LvarCollector, source_info: SourceInfoRef) -> Self {
        ParseResult {
            node,
            ident_table: IdentifierTable::new(),
            lvar_collector,
            source_info,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LvarId(usize);

impl std::ops::Deref for LvarId {
    type Target = usize;
    fn deref(&self) -> &usize {
        &self.0
    }
}

impl std::hash::Hash for LvarId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl LvarId {
    pub fn as_usize(&self) -> usize {
        self.0
    }

    pub fn as_u32(&self) -> u32 {
        self.0 as u32
    }

    pub fn from_usize(id: usize) -> Self {
        LvarId(id)
    }

    pub fn from_u32(id: u32) -> Self {
        LvarId(id as usize)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LvarCollector {
    id: usize,
    table: HashMap<IdentId, LvarId>,
    block: Option<LvarId>,
}

impl LvarCollector {
    pub fn new() -> Self {
        LvarCollector {
            id: 0,
            table: HashMap::new(),
            block: None,
        }
    }

    fn insert(&mut self, val: IdentId) -> LvarId {
        match self.table.get(&val) {
            Some(id) => *id,
            None => {
                let id = self.id;
                self.table.insert(val, LvarId(id));
                self.id += 1;
                LvarId(id)
            }
        }
    }

    fn insert_new(&mut self, val: IdentId) -> Result<LvarId, ()> {
        let id = self.id;
        if self.table.insert(val, LvarId(id)).is_some() {
            return Err(());
        };
        self.id += 1;
        Ok(LvarId(id))
    }

    fn insert_block_param(&mut self, val: IdentId) -> Result<LvarId, ()> {
        let lvar = self.insert_new(val)?;
        self.block = Some(lvar);
        Ok(lvar)
    }

    pub fn get(&self, val: &IdentId) -> Option<&LvarId> {
        self.table.get(val)
    }

    pub fn block_param(&self) -> Option<LvarId> {
        self.block
    }

    pub fn len(&self) -> usize {
        self.table.len()
    }

    pub fn table(&self) -> &HashMap<IdentId, LvarId> {
        &self.table
    }

    pub fn block(&self) -> &Option<LvarId> {
        &self.block
    }

    pub fn clone_table(&self) -> HashMap<IdentId, LvarId> {
        self.table.clone()
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Context {
    lvar: LvarCollector,
    kind: ContextKind,
}

impl Context {
    fn new_method() -> Self {
        Context {
            lvar: LvarCollector::new(),
            kind: ContextKind::Method,
        }
    }
    fn new_class(lvar_collector: Option<LvarCollector>) -> Self {
        Context {
            lvar: lvar_collector.unwrap_or(LvarCollector::new()),
            kind: ContextKind::Class,
        }
    }
    fn new_block() -> Self {
        Context {
            lvar: LvarCollector::new(),
            kind: ContextKind::Block,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum ContextKind {
    Class,
    Method,
    Block,
}

impl Parser {
    pub fn new() -> Self {
        let lexer = Lexer::new();
        Parser {
            lexer,
            tokens: vec![],
            cursor: 0,
            prev_cursor: 0,
            context_stack: vec![],
            ident_table: IdentifierTable::new(),
            state_save: vec![],
        }
    }

    fn save_state(&mut self) {
        self.state_save.push((self.cursor, self.prev_cursor));
    }

    fn restore_state(&mut self) {
        let state = self.state_save.pop().unwrap();
        self.cursor = state.0;
        self.prev_cursor = state.1;
    }

    fn discard_state(&mut self) {
        self.state_save.pop().unwrap();
    }

    pub fn get_context_depth(&self) -> usize {
        self.context_stack.len()
    }

    fn context_mut(&mut self) -> &mut Context {
        self.context_stack.last_mut().unwrap()
    }

    // If the identifier(IdentId) does not exist in the current scope,
    // add the identifier as a local variable in the current context.
    fn add_local_var_if_new(&mut self, id: IdentId) {
        if !self.is_local_var(id) {
            self.context_mut().lvar.insert(id);
        }
    }

    // Add the identifier(IdentId) as a new parameter in the current context.
    // If a parameter with the same name already exists, return error.
    fn new_param(&mut self, id: IdentId, loc: Loc) -> Result<(), RubyError> {
        let res = self.context_mut().lvar.insert_new(id);
        if res.is_err() {
            return Err(self.error_unexpected(loc, "Duplicated argument name."));
        }
        Ok(())
    }

    // Add the identifier(IdentId) as a new block parameter in the current context.
    // If a parameter with the same name already exists, return error.
    fn new_block_param(&mut self, id: IdentId, loc: Loc) -> Result<(), RubyError> {
        let res = self.context_mut().lvar.insert_block_param(id);
        if res.is_err() {
            return Err(self.error_unexpected(loc, "Duplicated argument name."));
        }
        Ok(())
    }

    // Examine whether the identifier(IdentId) exists in the current scope.
    // If exiets, return true.
    fn is_local_var(&mut self, id: IdentId) -> bool {
        let len = self.context_stack.len();
        for i in 0..len {
            let context = &self.context_stack[len - 1 - i];
            if context.lvar.table.contains_key(&id) {
                return true;
            }
            if context.kind != ContextKind::Block {
                return false;
            }
        }
        return false;
    }

    fn get_ident_id(&mut self, method: impl Into<String>) -> IdentId {
        self.ident_table.get_ident_id(method)
    }

    pub fn show_tokens(&self) {
        for tok in &self.tokens {
            eprintln!("{:?}", tok);
        }
    }

    /// Peek next token (skipping line terminators).
    fn peek(&self) -> &Token {
        let mut c = self.cursor;
        loop {
            let tok = &self.tokens[c];
            if tok.is_eof() || !tok.is_line_term() {
                return tok;
            } else {
                c += 1;
            }
        }
    }

    /// Peek next token (no skipping line terminators).
    fn peek_no_term(&self) -> &Token {
        &self.tokens[self.cursor]
    }

    /// Examine the next token, and return true if it is a line terminator.
    fn is_line_term(&self) -> bool {
        self.peek_no_term().is_line_term()
    }

    fn loc(&self) -> Loc {
        self.tokens[self.cursor].loc()
    }

    fn prev_loc(&self) -> Loc {
        self.tokens[self.prev_cursor].loc()
    }

    /// Get next token (skipping line terminators).
    /// Return RubyError if it was EOF.
    fn get(&mut self) -> Result<Token, RubyError> {
        loop {
            let token = self.tokens[self.cursor].clone();
            if token.is_eof() {
                return Err(self.error_eof(token.loc()));
            }
            self.prev_cursor = self.cursor;
            self.cursor += 1;
            if !token.is_line_term() {
                return Ok(token);
            }
        }
    }

    /// Get next token (no skipping line terminators).
    fn get_no_skip_line_term(&mut self) -> Token {
        let token = self.tokens[self.cursor].clone();
        if !token.is_eof() {
            self.prev_cursor = self.cursor;
            self.cursor += 1;
        }
        token
    }

    /// If next token is an expected kind of Punctuator, get it and return true.
    /// Otherwise, return false.
    fn consume_punct(&mut self, expect: Punct) -> bool {
        match &self.peek().kind {
            TokenKind::Punct(punct) if *punct == expect => {
                let _ = self.get();
                true
            }
            _ => false,
        }
    }

    fn consume_punct_no_term(&mut self, expect: Punct) -> bool {
        if TokenKind::Punct(expect) == self.peek_no_term().kind {
            let _ = self.get();
            true
        } else {
            false
        }
    }

    /// If next token is an expected kind of Reserved keyeord, get it and return true.
    /// Otherwise, return false.
    fn consume_reserved(&mut self, expect: Reserved) -> bool {
        match &self.peek().kind {
            TokenKind::Reserved(reserved) if *reserved == expect => {
                let _ = self.get();
                true
            }
            _ => false,
        }
    }

    fn consume_reserved_no_skip_line_term(&mut self, expect: Reserved) -> bool {
        if TokenKind::Reserved(expect) == self.peek_no_term().kind {
            let _ = self.get();
            true
        } else {
            false
        }
    }

    /// Get the next token if it is a line terminator or ';' or EOF, and return true,
    /// Otherwise, return false.
    fn consume_term(&mut self) -> bool {
        if !self.peek_no_term().is_term() {
            return false;
        };
        while self.peek_no_term().is_term() {
            if self.get_no_skip_line_term().is_eof() {
                return true;
            }
        }
        return true;
    }

    /// Get the next token and examine whether it is an expected Reserved.
    /// If not, return RubyError.
    fn expect_reserved(&mut self, expect: Reserved) -> Result<(), RubyError> {
        match &self.get()?.kind {
            TokenKind::Reserved(reserved) if *reserved == expect => Ok(()),
            _ => Err(self.error_unexpected(self.prev_loc(), format!("Expect {:?}", expect))),
        }
    }

    /// Get the next token and examine whether it is an expected Punct.
    /// If not, return RubyError.
    fn expect_punct(&mut self, expect: Punct) -> Result<(), RubyError> {
        match &self.get()?.kind {
            TokenKind::Punct(punct) if *punct == expect => Ok(()),
            _ => Err(self.error_unexpected(self.prev_loc(), format!("Expect '{:?}'", expect))),
        }
    }

    /// Get the next token and examine whether it is Ident.
    /// Return IdentId of the Ident.
    /// If not, return RubyError.
    fn expect_ident(&mut self) -> Result<IdentId, RubyError> {
        let name = match &self.get()?.kind {
            TokenKind::Ident(s, _) => s.clone(),
            _ => {
                return Err(self.error_unexpected(self.prev_loc(), "Expect identifier."));
            }
        };
        Ok(self.get_ident_id(name))
    }

    /// Get the next token and examine whether it is Const.
    /// Return IdentId of the Const.
    /// If not, return RubyError.
    fn expect_const(&mut self) -> Result<IdentId, RubyError> {
        let name = match &self.get()?.kind {
            TokenKind::Const(s) => s.clone(),
            _ => {
                return Err(self.error_unexpected(self.prev_loc(), "Expect constant."));
            }
        };
        Ok(self.get_ident_id(name))
    }

    fn token_as_symbol(&self, token: &Token) -> String {
        match token.kind.clone() {
            TokenKind::Ident(ident, _) => ident,
            TokenKind::Const(ident) => ident,
            TokenKind::InstanceVar(ident) => ident,
            TokenKind::StringLit(ident) => ident,
            TokenKind::Reserved(reserved) => {
                self.lexer.get_string_from_reserved(reserved).to_string()
            }
            _ => unreachable!(),
        }
    }

    fn error_unexpected(&self, loc: Loc, msg: impl Into<String>) -> RubyError {
        RubyError::new_parse_err(
            ParseErrKind::SyntaxError(msg.into()),
            self.lexer.source_info,
            0,
            loc,
        )
    }

    fn error_eof(&self, loc: Loc) -> RubyError {
        RubyError::new_parse_err(ParseErrKind::UnexpectedEOF, self.lexer.source_info, 0, loc)
    }
}

impl Parser {
    pub fn parse_program(
        mut self,
        path: impl Into<String>,
        program: String,
    ) -> Result<ParseResult, RubyError> {
        self.lexer.source_info.path = path.into();
        self.tokens = self.lexer.tokenize(program.clone())?.tokens;
        self.cursor = 0;
        self.prev_cursor = 0;
        self.context_stack.push(Context::new_class(None));
        let node = self.parse_comp_stmt()?;
        let lvar = self.context_stack.pop().unwrap().lvar;

        let tok = self.peek();
        if tok.kind == TokenKind::EOF {
            let mut result = ParseResult::default(node, lvar, self.lexer.source_info);
            result.ident_table = self.ident_table;
            Ok(result)
        } else {
            Err(self.error_unexpected(tok.loc(), "Expected end-of-input."))
        }
    }

    pub fn parse_program_repl(
        mut self,
        path: impl Into<String>,
        program: String,
        lvar_collector: Option<LvarCollector>,
    ) -> Result<ParseResult, RubyError> {
        self.lexer.source_info.path = path.into();
        self.tokens = self.lexer.tokenize(program.clone())?.tokens;
        self.cursor = 0;
        self.prev_cursor = 0;
        self.context_stack.push(Context::new_class(lvar_collector));
        let node = match self.parse_comp_stmt() {
            Ok(node) => node,
            Err(mut err) => {
                err.set_level(self.context_stack.len() - 1);
                return Err(err);
            }
        };
        let lvar = self.context_stack.pop().unwrap().lvar;

        let tok = self.peek();
        if tok.kind == TokenKind::EOF {
            let mut result = ParseResult::default(node, lvar, self.lexer.source_info);
            std::mem::swap(&mut result.ident_table, &mut self.ident_table);
            Ok(result)
        } else {
            let mut err = self.error_unexpected(tok.loc(), "Expected end-of-input.");
            err.set_level(0);
            Err(err)
        }
    }

    fn parse_comp_stmt(&mut self) -> Result<Node, RubyError> {
        // COMP_STMT : (STMT (TERM STMT)*)? (TERM+)?
        /*
        fn check_stmt_end(token: &Token) -> bool {
            match token.kind {
                TokenKind::EOF
                | TokenKind::IntermediateDoubleQuote(_)
                | TokenKind::CloseDoubleQuote(_) => true,
                TokenKind::Reserved(reserved) => match reserved {
                    Reserved::Else | Reserved::Elsif | Reserved::End | Reserved::When => true,
                    _ => false,
                },
                TokenKind::Punct(punct) => match punct {
                    Punct::RParen | Punct::RBrace | Punct::RBracket => true,
                    _ => false,
                },
                _ => false,
            }
        }*/

        let loc = self.loc();
        let mut nodes = vec![];

        loop {
            if self.peek().check_stmt_end() {
                return Ok(Node::new_comp_stmt(nodes, loc));
            }

            let node = self.parse_stmt()?;
            //println!("node {:?}", node);
            nodes.push(node);
            if !self.consume_term() {
                break;
            }
        }
        Ok(Node::new_comp_stmt(nodes, loc))
    }

    fn parse_stmt(&mut self) -> Result<Node, RubyError> {
        let mut node = self.parse_expr()?;
        loop {
            if self.consume_reserved_no_skip_line_term(Reserved::If) {
                // STMT : STMT if EXPR
                let loc = self.prev_loc();
                let cond = self.parse_expr()?;
                node = Node::new_if(cond, node, Node::new_comp_stmt(vec![], loc), loc);
            } else if self.consume_reserved_no_skip_line_term(Reserved::Unless) {
                // STMT : STMT unless EXPR
                let loc = self.prev_loc();
                let cond = self.parse_expr()?;
                node = Node::new_if(cond, Node::new_comp_stmt(vec![], loc), node, loc);
            } else if self.consume_reserved_no_skip_line_term(Reserved::While) {
                // STMT : STMT while EXPR
                let loc = self.prev_loc();
                let cond = self.parse_expr()?;
                let loc = loc.merge(self.prev_loc());
                node = Node::new_while(cond, node, loc);
            } else if self.consume_reserved_no_skip_line_term(Reserved::Until) {
                // STMT : STMT until EXPR
                let loc = self.prev_loc();
                let cond = Node::new_unop(UnOp::Not, self.parse_expr()?, loc);
                let loc = loc.merge(self.prev_loc());
                node = Node::new_while(cond, node, loc);
            } else {
                break;
            }
        }
        // STMT : EXPR
        Ok(node)
    }

    fn parse_expr(&mut self) -> Result<Node, RubyError> {
        // EXPR : NOT
        // | KEYWORD-AND
        // | KEYWORD-OR
        // NOT : ARG
        // | UNPARENTHESIZED-METHOD
        // | ! UNPARENTHESIZED-METHOD
        // | KEYWORD-NOT
        // UNPARENTHESIZED-METHOD :
        // | FNAME ARGS
        // | PRIMARY . FNAME ARGS
        // | PRIMARY :: FNAME ARGS
        // | return ARGS
        // | break ARGS
        // | next ARGS
        // | COMMAND-WITH-DO-BLOCK [CHAIN-METHOD]*
        // | COMMAND-WITH-DO-BLOCK [CHAIN-METHOD]* . FNAME ARGS
        // | COMMAND-WITH-DO-BLOCK [CHAIN-METHOD]* :: FNAME ARGS
        // CHAIN-METOD : . FNAME
        // | :: FNAME
        // | . FNAME( ARGS )
        // | :: FNAME( ARGS )
        // COMMAND-WITH-DO-BLOCK : FNAME ARGS DO-BLOCK
        // | PRIMARY . FNAME ARGS DO-BLOCK [CHAIN-METHOD]* [ . FNAME ARGS]
        let node = self.parse_arg()?;
        if self.consume_punct_no_term(Punct::Comma)
        /*&& node.is_lvar()*/
        {
            // EXPR : MLHS `=' MRHS
            return Ok(self.parse_mul_assign(node)?);
        }
        if node.is_operation() && self.is_command() {
            // FNAME ARGS
            // FNAME ARGS DO-BLOCK
            Ok(self.parse_command(node.as_method_name().unwrap(), node.loc())?)
        } else if let Node {
            // PRIMARY . FNAME ARGS
            // PRIMARY . FNAME ARGS DO_BLOCK [CHAIN-METHOD]* [ . FNAME ARGS]
            kind:
                NodeKind::Send {
                    method,
                    receiver,
                    mut args,
                    completed: false,
                    ..
                },
            mut loc,
        } = node.clone()
        {
            let mut kw_args = vec![];
            if self.is_command() {
                let res = self.parse_arglist()?;
                args = res.0;
                kw_args = res.1;
                loc = loc.merge(args[0].loc());
            }
            let block = self.parse_block()?;
            let node = Node::new_send(*receiver, method, args, kw_args, block, true, loc);
            Ok(node)
        } else {
            // EXPR : ARG
            Ok(node)
        }
    }

    fn parse_mul_assign(&mut self, node: Node) -> Result<Node, RubyError> {
        // EXPR : MLHS `=' MRHS
        let mut new_lvar = vec![];
        if let NodeKind::Ident(id, has_suffix) = node.kind {
            if has_suffix {
                return Err(self.error_unexpected(node.loc(), "Illegal identifier for left hand."));
            };
            new_lvar.push(id);
        };
        let mut mlhs = vec![node];
        loop {
            if self.peek_no_term().kind == TokenKind::Punct(Punct::Assign) {
                break;
            }
            let node = self.parse_function()?;
            if let NodeKind::Ident(id, has_suffix) = node.kind {
                if has_suffix {
                    return Err(
                        self.error_unexpected(node.loc(), "Illegal identifier for left hand.")
                    );
                };
                new_lvar.push(id);
            };
            mlhs.push(node);
            if !self.consume_punct_no_term(Punct::Comma) {
                break;
            }
        }

        if !self.consume_punct_no_term(Punct::Assign) {
            return Err(self.error_unexpected(self.loc(), "Expected '='."));
        }

        let (mrhs, _) = self.parse_args(None)?;
        for lvar in new_lvar {
            self.add_local_var_if_new(lvar);
        }
        return Ok(Node::new_mul_assign(mlhs, mrhs));
    }

    fn parse_command(&mut self, operation: IdentId, loc: Loc) -> Result<Node, RubyError> {
        // FNAME ARGS
        // FNAME ARGS DO-BLOCK
        let (args, kw_args) = self.parse_arglist()?;
        let block = self.parse_block()?;
        Ok(Node::new_send(
            Node::new_self(loc),
            operation,
            args,
            kw_args,
            block,
            true,
            loc,
        ))
    }

    fn parse_arglist(&mut self) -> Result<(Vec<Node>, Vec<(IdentId, Node)>), RubyError> {
        let first_arg = self.parse_arg()?;

        if first_arg.is_operation() && self.is_command() {
            let nodes =
                vec![self.parse_command(first_arg.as_method_name().unwrap(), first_arg.loc())?];
            return Ok((nodes, vec![]));
        }

        let mut args = vec![first_arg];
        let mut kw_args = vec![];
        if self.consume_punct(Punct::Comma) {
            let res = self.parse_args(None)?;
            let mut new_args = res.0;
            kw_args = res.1;
            args.append(&mut new_args);
        }
        Ok((args, kw_args))
    }

    fn is_command(&mut self) -> bool {
        let tok = self.peek_no_term();
        match tok.kind {
            TokenKind::Ident(_, _)
            | TokenKind::InstanceVar(_)
            | TokenKind::Const(_)
            | TokenKind::NumLit(_)
            | TokenKind::FloatLit(_)
            | TokenKind::StringLit(_)
            | TokenKind::OpenDoubleQuote(_) => true,
            TokenKind::Punct(p) => match p {
                Punct::LParen
                | Punct::LBracket
                | Punct::LBrace
                | Punct::Colon
                | Punct::Scope
                | Punct::Plus
                | Punct::Minus
                | Punct::Arrow => true,
                _ => false,
            },
            TokenKind::Reserved(r) => match r {
                Reserved::False | Reserved::Nil | Reserved::True => true,
                _ => false,
            },
            _ => false,
        }
    }

    fn parse_arg(&mut self) -> Result<Node, RubyError> {
        self.parse_arg_assign()
    }

    fn parse_arg_assign(&mut self) -> Result<Node, RubyError> {
        let mut lhs = self.parse_arg_ternary()?;
        if self.is_line_term() {
            return Ok(lhs);
        }
        if self.consume_punct_no_term(Punct::Assign) {
            let (mrhs, _) = self.parse_args(None)?;
            self.check_lhs(&lhs)?;
            Ok(Node::new_mul_assign(vec![lhs], mrhs))
        } else if let TokenKind::Punct(Punct::AssignOp(op)) = self.peek_no_term().kind {
            match op {
                BinOp::LOr => {
                    self.get()?;
                    let rhs = self.parse_arg()?;
                    self.check_lhs(&lhs)?;
                    if let NodeKind::Ident(id, _) = lhs.kind {
                        lhs = Node::new_lvar(id, lhs.loc());
                    };
                    let node = Node::new_binop(
                        BinOp::LOr,
                        lhs.clone(),
                        Node::new_mul_assign(vec![lhs.clone()], vec![rhs]),
                    );
                    Ok(node)
                }
                _ => {
                    //let loc = self.loc();
                    self.get()?;
                    let rhs = self.parse_arg()?;
                    self.check_lhs(&lhs)?;
                    Ok(Node::new_mul_assign(
                        vec![lhs.clone()],
                        vec![Node::new_binop(op, lhs, rhs)],
                    ))
                }
            }
        } else {
            Ok(lhs)
        }
    }

    fn check_lhs(&mut self, lhs: &Node) -> Result<(), RubyError> {
        if let NodeKind::Ident(id, has_suffix) = lhs.kind {
            if has_suffix {
                return Err(self.error_unexpected(lhs.loc(), "Illegal identifier for left hand."));
            };
            self.add_local_var_if_new(id);
        };
        Ok(())
    }

    fn parse_arg_ternary(&mut self) -> Result<Node, RubyError> {
        let cond = self.parse_arg_range()?;
        let loc = cond.loc();
        if self.consume_punct_no_term(Punct::Question) {
            let then_ = self.parse_arg()?;
            if !self.consume_punct_no_term(Punct::Colon) {
                return Err(self.error_unexpected(self.loc(), "Expect ':'."));
            };
            let else_ = self.parse_arg()?;
            let node = Node::new_if(cond, then_, else_, loc);
            Ok(node)
        } else {
            Ok(cond)
        }
    }

    fn parse_arg_range(&mut self) -> Result<Node, RubyError> {
        let lhs = self.parse_arg_logical_or()?;
        if self.is_line_term() {
            return Ok(lhs);
        }
        if self.consume_punct(Punct::Range2) {
            let rhs = self.parse_arg_logical_or()?;
            let loc = lhs.loc().merge(rhs.loc());
            Ok(Node::new_range(lhs, rhs, false, loc))
        } else if self.consume_punct(Punct::Range3) {
            let rhs = self.parse_arg_logical_or()?;
            let loc = lhs.loc().merge(rhs.loc());
            Ok(Node::new_range(lhs, rhs, true, loc))
        } else {
            Ok(lhs)
        }
    }

    fn parse_arg_logical_or(&mut self) -> Result<Node, RubyError> {
        let mut lhs = self.parse_arg_logical_and()?;
        while self.consume_punct_no_term(Punct::LOr) {
            let rhs = self.parse_arg_logical_and()?;
            lhs = Node::new_binop(BinOp::LOr, lhs, rhs);
        }
        Ok(lhs)
    }

    fn parse_arg_logical_and(&mut self) -> Result<Node, RubyError> {
        let mut lhs = self.parse_arg_eq()?;
        while self.consume_punct_no_term(Punct::LAnd) {
            let rhs = self.parse_arg_eq()?;
            lhs = Node::new_binop(BinOp::LAnd, lhs, rhs);
        }
        Ok(lhs)
    }

    // 4==4==4 => SyntaxError
    fn parse_arg_eq(&mut self) -> Result<Node, RubyError> {
        let lhs = self.parse_arg_comp()?;
        // TODO: Support <==> === =~ !~
        if self.consume_punct_no_term(Punct::Eq) {
            let rhs = self.parse_arg_comp()?;
            Ok(Node::new_binop(BinOp::Eq, lhs, rhs))
        } else if self.consume_punct_no_term(Punct::Ne) {
            let rhs = self.parse_arg_comp()?;
            Ok(Node::new_binop(BinOp::Ne, lhs, rhs))
        } else if self.consume_punct_no_term(Punct::TEq) {
            let rhs = self.parse_arg_comp()?;
            Ok(Node::new_binop(BinOp::TEq, lhs, rhs))
        } else {
            Ok(lhs)
        }
    }

    fn parse_arg_comp(&mut self) -> Result<Node, RubyError> {
        let mut lhs = self.parse_arg_bitor()?;
        if self.is_line_term() {
            return Ok(lhs);
        }
        loop {
            if self.consume_punct_no_term(Punct::Ge) {
                let rhs = self.parse_arg_bitor()?;
                lhs = Node::new_binop(BinOp::Ge, lhs, rhs);
            } else if self.consume_punct_no_term(Punct::Gt) {
                let rhs = self.parse_arg_bitor()?;
                lhs = Node::new_binop(BinOp::Gt, lhs, rhs);
            } else if self.consume_punct_no_term(Punct::Le) {
                let rhs = self.parse_arg_bitor()?;
                lhs = Node::new_binop(BinOp::Le, lhs, rhs);
            } else if self.consume_punct_no_term(Punct::Lt) {
                let rhs = self.parse_arg_bitor()?;
                lhs = Node::new_binop(BinOp::Lt, lhs, rhs);
            } else {
                break;
            }
        }
        Ok(lhs)
    }

    fn parse_arg_bitor(&mut self) -> Result<Node, RubyError> {
        let mut lhs = self.parse_arg_bitand()?;
        loop {
            if self.consume_punct_no_term(Punct::BitOr) {
                lhs = Node::new_binop(BinOp::BitOr, lhs, self.parse_arg_bitand()?);
            } else if self.consume_punct_no_term(Punct::BitXor) {
                lhs = Node::new_binop(BinOp::BitXor, lhs, self.parse_arg_bitand()?);
            } else {
                break;
            }
        }
        Ok(lhs)
    }

    fn parse_arg_bitand(&mut self) -> Result<Node, RubyError> {
        let mut lhs = self.parse_arg_shift()?;
        loop {
            if self.consume_punct_no_term(Punct::BitAnd) {
                lhs = Node::new_binop(BinOp::BitAnd, lhs, self.parse_arg_shift()?);
            } else {
                break;
            }
        }
        Ok(lhs)
    }

    fn parse_arg_shift(&mut self) -> Result<Node, RubyError> {
        let mut lhs = self.parse_arg_add()?;
        loop {
            if self.consume_punct_no_term(Punct::Shl) {
                lhs = Node::new_binop(BinOp::Shl, lhs, self.parse_arg_add()?);
            } else if self.consume_punct_no_term(Punct::Shr) {
                lhs = Node::new_binop(BinOp::Shr, lhs, self.parse_arg_add()?);
            } else {
                break;
            }
        }
        Ok(lhs)
    }

    fn parse_arg_add(&mut self) -> Result<Node, RubyError> {
        let mut lhs = self.parse_arg_mul()?;
        loop {
            if self.consume_punct_no_term(Punct::Plus) {
                let rhs = self.parse_arg_mul()?;
                lhs = Node::new_binop(BinOp::Add, lhs, rhs);
            } else if self.consume_punct_no_term(Punct::Minus) {
                let rhs = self.parse_arg_mul()?;
                lhs = Node::new_binop(BinOp::Sub, lhs, rhs);
            } else {
                break;
            }
        }
        Ok(lhs)
    }

    fn parse_arg_mul(&mut self) -> Result<Node, RubyError> {
        let mut lhs = self.parse_unary_minus()?;
        if self.is_line_term() {
            return Ok(lhs);
        }
        loop {
            if self.consume_punct_no_term(Punct::Mul) {
                let rhs = self.parse_unary_minus()?;
                lhs = Node::new_binop(BinOp::Mul, lhs, rhs);
            } else if self.consume_punct_no_term(Punct::Div) {
                let rhs = self.parse_unary_minus()?;
                lhs = Node::new_binop(BinOp::Div, lhs, rhs);
            } else if self.consume_punct_no_term(Punct::Rem) {
                let rhs = self.parse_unary_minus()?;
                lhs = Node::new_binop(BinOp::Rem, lhs, rhs);
            } else {
                break;
            }
        }
        Ok(lhs)
    }

    fn parse_unary_minus(&mut self) -> Result<Node, RubyError> {
        self.save_state();
        if self.consume_punct(Punct::Minus) {
            let loc = self.prev_loc();
            match self.peek().kind {
                TokenKind::NumLit(_) | TokenKind::FloatLit(_) => {
                    self.restore_state();
                    let lhs = self.parse_exponent()?;
                    return Ok(lhs);
                }
                _ => self.discard_state(),
            };
            let lhs = self.parse_unary_minus()?;
            let loc = loc.merge(lhs.loc());
            let lhs = Node::new_binop(BinOp::Mul, lhs, Node::new_integer(-1, loc));
            Ok(lhs)
        } else {
            self.discard_state();
            let lhs = self.parse_exponent()?;
            Ok(lhs)
        }
    }

    fn parse_exponent(&mut self) -> Result<Node, RubyError> {
        let lhs = self.parse_unary()?;
        if self.consume_punct_no_term(Punct::DMul) {
            let rhs = self.parse_exponent()?;
            Ok(Node::new_binop(BinOp::Exp, lhs, rhs))
        } else {
            Ok(lhs)
        }
    }

    fn parse_unary(&mut self) -> Result<Node, RubyError> {
        // TODO: Support unary '+'.
        if self.consume_punct(Punct::BitNot) {
            let loc = self.prev_loc();
            let lhs = self.parse_unary()?;
            let lhs = Node::new_unop(UnOp::BitNot, lhs, loc);
            Ok(lhs)
        } else if self.consume_punct(Punct::Not) {
            let loc = self.prev_loc();
            let lhs = self.parse_unary()?;
            let lhs = Node::new_unop(UnOp::Not, lhs, loc);
            Ok(lhs)
        } else {
            let lhs = self.parse_function()?;
            Ok(lhs)
        }
    }

    fn parse_function(&mut self) -> Result<Node, RubyError> {
        // <一次式メソッド呼び出し>
        let mut node = self.parse_primary()?;
        let loc = node.loc();
        if node.is_operation() {
            if self.consume_punct_no_term(Punct::LParen) {
                // PRIMARY-METHOD : FNAME ( ARGS ) BLOCK?
                let (args, kw_args) = self.parse_args(Punct::RParen)?;
                let block = self.parse_block()?;

                node = Node::new_send(
                    Node::new_self(loc),
                    node.as_method_name().unwrap(),
                    args,
                    kw_args,
                    block,
                    true,
                    loc,
                );
            } else if let Some(block) = self.parse_block()? {
                // PRIMARY-METHOD : FNAME BLOCK
                node = Node::new_send(
                    Node::new_self(loc),
                    node.as_method_name().unwrap(),
                    vec![],
                    vec![],
                    Some(block),
                    true,
                    loc,
                );
            }
        }
        loop {
            let tok = self.peek();
            node = match tok.kind {
                TokenKind::Punct(Punct::Dot) => {
                    // PRIMARY-METHOD :
                    // | PRIMARY . FNAME BLOCK => completed: true
                    // | PRIMARY . FNAME ( ARGS ) BLOCK? => completed: true
                    // | PRIMARY . FNAME => completed: false
                    self.get()?;
                    let tok = self.get()?.clone();
                    let method = match &tok.kind {
                        TokenKind::Ident(s, has_suffix) => {
                            if *has_suffix {
                                match self.get()?.kind {
                                    TokenKind::Punct(Punct::Question) => s.clone() + "?",
                                    TokenKind::Punct(Punct::Not) => s.clone() + "!",
                                    _ => {
                                        return Err(
                                            self.error_unexpected(tok.loc, "Illegal method name.")
                                        )
                                    }
                                }
                            } else {
                                s.clone()
                            }
                        }
                        TokenKind::Reserved(r) => {
                            let string = self.lexer.get_string_from_reserved(*r);
                            string.clone()
                        }
                        _ => {
                            return Err(self
                                .error_unexpected(tok.loc(), "method name must be an identifier."))
                        }
                    };
                    let id = self.get_ident_id(method);
                    let mut args = vec![];
                    let mut kw_args = vec![];
                    let mut completed = false;
                    if self.consume_punct_no_term(Punct::LParen) {
                        let res = self.parse_args(Punct::RParen)?;
                        args = res.0;
                        kw_args = res.1;
                        completed = true;
                    }
                    let block = self.parse_block()?;
                    if block.is_some() {
                        completed = true;
                    };
                    let node = match node.kind {
                        NodeKind::Ident(id, _) => {
                            Node::new_send(Node::new_self(loc), id, vec![], vec![], None, true, loc)
                        }
                        _ => node,
                    };
                    Node::new_send(
                        node,
                        id,
                        args,
                        kw_args,
                        block,
                        completed,
                        loc.merge(self.loc()),
                    )
                }
                TokenKind::Punct(Punct::LBracket) => {
                    if node.is_operation() {
                        return Ok(node);
                    };
                    self.get()?;
                    let (mut args, _) = self.parse_args(Punct::RBracket)?;
                    args.reverse();
                    Node::new_array_member(node, args)
                }
                TokenKind::Punct(Punct::Scope) => {
                    self.get()?;
                    let loc = self.loc();
                    let id = self.expect_const()?;
                    Node::new_scope(node, id, loc)
                }
                _ => return Ok(node),
            }
        }
    }

    /// Parse argument list.
    /// punct: punctuator for terminating arg list. Set None for unparenthesized argument list.
    fn parse_args(
        &mut self,
        punct: impl Into<Option<Punct>>,
    ) -> Result<(Vec<Node>, Vec<(IdentId, Node)>), RubyError> {
        let (flag, punct) = match punct.into() {
            Some(punct) => (true, punct),
            None => (false, Punct::Arrow /* dummy */),
        };
        let mut args = vec![];
        let mut keyword_args = vec![];
        loop {
            if flag && self.consume_punct(punct) {
                return Ok((args, keyword_args));
            }
            if self.consume_punct(Punct::Mul) {
                // splat argument
                let loc = self.prev_loc();
                let array = self.parse_arg()?;
                args.push(Node::new_splat(array, loc));
            } else {
                let node = self.parse_arg()?;
                match node.kind {
                    NodeKind::Ident(id, ..) | NodeKind::LocalVar(id) => {
                        if self.consume_punct_no_term(Punct::Colon) {
                            keyword_args.push((id, self.parse_arg()?));
                        } else {
                            args.push(node);
                        }
                    }
                    _ => {
                        args.push(node);
                    }
                }
            }
            if !self.consume_punct(Punct::Comma) {
                break;
            }
        }
        if flag {
            self.expect_punct(punct)?
        };
        Ok((args, keyword_args))
    }

    fn parse_block(&mut self) -> Result<Option<Box<Node>>, RubyError> {
        let do_flag = if self.consume_reserved_no_skip_line_term(Reserved::Do) {
            true
        } else {
            if self.consume_punct_no_term(Punct::LBrace) {
                false
            } else {
                return Ok(None);
            }
        };
        // BLOCK: do [`|' [BLOCK_VAR] `|'] COMPSTMT end
        let loc = self.prev_loc();
        self.context_stack.push(Context::new_block());
        let mut params = vec![];
        if self.consume_punct(Punct::BitOr) {
            if !self.consume_punct(Punct::BitOr) {
                loop {
                    let id = self.expect_ident()?;
                    params.push(Node::new_param(id, self.prev_loc()));
                    self.new_param(id, self.prev_loc())?;
                    if !self.consume_punct(Punct::Comma) {
                        break;
                    }
                }
                self.expect_punct(Punct::BitOr)?;
            }
        } else {
            self.consume_punct(Punct::LOr);
        }
        let body = self.parse_comp_stmt()?;
        if do_flag {
            self.expect_reserved(Reserved::End)?;
        } else {
            self.expect_punct(Punct::RBrace)?;
        };
        let lvar = self.context_stack.pop().unwrap().lvar;
        let loc = loc.merge(self.prev_loc());
        let node = Node::new_proc(params, body, lvar, loc);
        Ok(Some(Box::new(node)))
    }

    fn parse_primary(&mut self) -> Result<Node, RubyError> {
        let tok = self.get()?.clone();
        let loc = tok.loc();
        match &tok.kind {
            TokenKind::Ident(name, has_suffix) => {
                let id = self.get_ident_id(name);
                if *has_suffix {
                    if self.consume_punct(Punct::Question) {
                        let id = self.get_ident_id(name.clone() + "?");
                        Ok(Node::new_identifier(id, true, loc.merge(self.prev_loc())))
                    } else if self.consume_punct(Punct::Not) {
                        let id = self.get_ident_id(name.clone() + "!");
                        Ok(Node::new_identifier(id, true, loc.merge(self.prev_loc())))
                    } else {
                        Ok(Node::new_identifier(id, true, loc))
                    }
                } else if self.is_local_var(id) {
                    Ok(Node::new_lvar(id, loc))
                } else {
                    // FUNCTION or COMMAND or LHS for assignment
                    Ok(Node::new_identifier(id, false, loc))
                }
            }
            TokenKind::InstanceVar(name) => {
                let id = self.get_ident_id(name);
                return Ok(Node::new_instance_var(id, loc));
            }
            TokenKind::GlobalVar(name) => {
                let id = self.get_ident_id(name);
                return Ok(Node::new_global_var(id, loc));
            }
            TokenKind::Const(name) => {
                let id = self.get_ident_id(name);
                Ok(Node::new_const(id, false, loc))
            }
            TokenKind::NumLit(num) => Ok(Node::new_integer(*num, loc)),
            TokenKind::FloatLit(num) => Ok(Node::new_float(*num, loc)),
            TokenKind::StringLit(s) => Ok(self.parse_string_literal(s)?),
            TokenKind::OpenDoubleQuote(s) => Ok(self.parse_interporated_string_literal(s)?),
            TokenKind::Punct(punct) => match punct {
                Punct::Minus => match self.get()?.kind {
                    TokenKind::NumLit(num) => Ok(Node::new_integer(-num, loc)),
                    TokenKind::FloatLit(num) => Ok(Node::new_float(-num, loc)),
                    _ => unreachable!(),
                },
                Punct::LParen => {
                    let node = self.parse_comp_stmt()?;
                    self.expect_punct(Punct::RParen)?;
                    Ok(node)
                }
                Punct::LBracket => {
                    let (mut nodes, _) = self.parse_args(Punct::RBracket)?;
                    nodes.reverse();
                    let loc = loc.merge(self.prev_loc());
                    Ok(Node::new_array(nodes, loc))
                }
                Punct::LBrace => self.parse_hash_literal(),
                Punct::Colon => {
                    let symbol_loc = self.loc();
                    let token = self.get()?;
                    let id = match &token.kind {
                        TokenKind::Punct(punct) => self.parse_op_definable(punct)?,
                        _ if token.can_be_symbol() => {
                            let ident = self.token_as_symbol(&token);
                            self.get_ident_id(ident)
                        }
                        _ => {
                            if let TokenKind::OpenDoubleQuote(s) = token.kind {
                                let node = self.parse_interporated_string_literal(&s)?;
                                let method = self.ident_table.get_ident_id("to_sym");
                                let loc = symbol_loc.merge(node.loc());
                                return Ok(Node::new_send(
                                    node,
                                    method,
                                    vec![],
                                    vec![],
                                    None,
                                    true,
                                    loc,
                                ));
                            }
                            return Err(
                                self.error_unexpected(symbol_loc, "Expect identifier or string.")
                            );
                        }
                    };
                    Ok(Node::new_symbol(id, loc.merge(self.prev_loc())))
                }
                Punct::Arrow => {
                    let mut params = vec![];
                    self.context_stack.push(Context::new_block());
                    if self.consume_punct(Punct::LParen) {
                        if !self.consume_punct(Punct::RParen) {
                            loop {
                                let id = self.expect_ident()?;
                                params.push(Node::new_param(id, self.prev_loc()));
                                self.new_param(id, self.prev_loc())?;
                                if !self.consume_punct(Punct::Comma) {
                                    break;
                                }
                            }
                            self.expect_punct(Punct::RParen)?;
                        }
                    } else if let TokenKind::Ident(_, _) = self.peek().kind {
                        let id = self.expect_ident()?;
                        self.new_param(id, self.prev_loc())?;
                        params.push(Node::new_param(id, self.prev_loc()));
                    };
                    self.expect_punct(Punct::LBrace)?;
                    let body = self.parse_comp_stmt()?;
                    self.expect_punct(Punct::RBrace)?;
                    let lvar = self.context_stack.pop().unwrap().lvar;
                    Ok(Node::new_proc(params, body, lvar, loc))
                }
                Punct::Scope => {
                    let id = self.expect_const()?;
                    Ok(Node::new_const(id, true, loc))
                }
                _ => {
                    return Err(
                        self.error_unexpected(loc, format!("Unexpected token: {:?}", tok.kind))
                    )
                }
            },
            TokenKind::Reserved(Reserved::If) => {
                let node = self.parse_if_then()?;
                self.expect_reserved(Reserved::End)?;
                Ok(node)
            }
            TokenKind::Reserved(Reserved::Unless) => {
                let node = self.parse_unless()?;
                self.expect_reserved(Reserved::End)?;
                Ok(node)
            }
            TokenKind::Reserved(Reserved::For) => {
                let loc = self.prev_loc();
                let var_id = self.expect_ident()?;
                let var = Node::new_lvar(var_id, self.prev_loc());
                self.add_local_var_if_new(var_id);
                self.expect_reserved(Reserved::In)?;
                let iter = self.parse_expr()?;
                self.parse_do()?;
                let body = self.parse_comp_stmt()?;
                self.expect_reserved(Reserved::End)?;
                let node = Node::new(
                    NodeKind::For {
                        param: Box::new(var),
                        iter: Box::new(iter),
                        body: Box::new(body),
                    },
                    loc.merge(self.prev_loc()),
                );
                Ok(node)
            }
            TokenKind::Reserved(Reserved::While) => {
                let loc = self.prev_loc();
                let cond = self.parse_expr()?;
                self.parse_do()?;
                let body = self.parse_comp_stmt()?;
                self.expect_reserved(Reserved::End)?;
                let loc = loc.merge(self.prev_loc());
                Ok(Node::new_while(cond, body, loc))
            }
            TokenKind::Reserved(Reserved::Until) => {
                let loc = self.prev_loc();
                let cond = self.parse_expr()?;
                let cond = Node::new_unop(UnOp::Not, cond, loc);
                self.parse_do()?;
                let body = self.parse_comp_stmt()?;
                self.expect_reserved(Reserved::End)?;
                let loc = loc.merge(self.prev_loc());
                Ok(Node::new_while(cond, body, loc))
            }
            TokenKind::Reserved(Reserved::Case) => {
                let loc = self.prev_loc();
                let cond = self.parse_expr()?;
                self.consume_term();
                let mut when_ = vec![];
                while self.consume_reserved(Reserved::When) {
                    let (arg, _) = self.parse_args(None)?;
                    self.parse_then()?;
                    let body = self.parse_comp_stmt()?;
                    when_.push(CaseBranch::new(arg, body));
                }
                let else_ = if self.consume_reserved(Reserved::Else) {
                    self.parse_comp_stmt()?
                } else {
                    Node::new_comp_stmt(vec![], self.loc())
                };
                self.expect_reserved(Reserved::End)?;
                Ok(Node::new_case(cond, when_, else_, loc))
            }
            TokenKind::Reserved(Reserved::Def) => Ok(self.parse_def()?),
            TokenKind::Reserved(Reserved::Class) => {
                if self.context_stack.last().unwrap().kind == ContextKind::Method {
                    return Err(
                        self.error_unexpected(loc, "SyntaxError: class definition in method body.")
                    );
                }
                Ok(self.parse_class(false)?)
            }
            TokenKind::Reserved(Reserved::Module) => {
                if self.context_stack.last().unwrap().kind == ContextKind::Method {
                    return Err(
                        self.error_unexpected(loc, "SyntaxError: class definition in method body.")
                    );
                }
                Ok(self.parse_class(true)?)
            }
            TokenKind::Reserved(Reserved::Return) => {
                let tok = self.peek_no_term();
                // TODO: This is not correct.
                if tok.is_term()
                    || tok.kind == TokenKind::Reserved(Reserved::Unless)
                    || tok.kind == TokenKind::Reserved(Reserved::If)
                    || tok.check_stmt_end()
                {
                    let val = Node::new_comp_stmt(vec![], loc);
                    return Ok(Node::new_return(val, loc));
                };
                let val = self.parse_arg()?;
                let ret_loc = val.loc();
                if self.consume_punct_no_term(Punct::Comma) {
                    let mut vec = vec![val, self.parse_arg()?];
                    while self.consume_punct_no_term(Punct::Comma) {
                        vec.push(self.parse_arg()?);
                    }
                    vec.reverse();
                    let val = Node::new_array(vec, ret_loc);
                    Ok(Node::new_return(val, loc))
                } else {
                    Ok(Node::new_return(val, loc))
                }
            }
            TokenKind::Reserved(Reserved::Break) => Ok(Node::new_break(loc)),
            TokenKind::Reserved(Reserved::Next) => Ok(Node::new_next(loc)),
            TokenKind::Reserved(Reserved::True) => Ok(Node::new_bool(true, loc)),
            TokenKind::Reserved(Reserved::False) => Ok(Node::new_bool(false, loc)),
            TokenKind::Reserved(Reserved::Nil) => Ok(Node::new_nil(loc)),
            TokenKind::Reserved(Reserved::Self_) => Ok(Node::new_self(loc)),
            TokenKind::Reserved(Reserved::Begin) => {
                let node = self.parse_comp_stmt()?;
                self.expect_reserved(Reserved::End)?;
                Ok(node)
            }
            TokenKind::EOF => return Err(self.error_eof(loc)),
            _ => {
                return Err(self.error_unexpected(loc, format!("Unexpected token: {:?}", tok.kind)))
            }
        }
    }

    fn parse_string_literal(&mut self, s: &String) -> Result<Node, RubyError> {
        let loc = self.prev_loc();
        let mut s = s.clone();
        while let TokenKind::StringLit(next_s) = &self.peek_no_term().clone().kind {
            self.get()?;
            s = format!("{}{}", s, next_s);
        }
        Ok(Node::new_string(s, loc))
    }

    fn parse_interporated_string_literal(&mut self, s: &String) -> Result<Node, RubyError> {
        let start_loc = self.prev_loc();
        let mut nodes = vec![Node::new_string(s.clone(), start_loc)];
        loop {
            match &self.peek().kind {
                TokenKind::CloseDoubleQuote(s) => {
                    let end_loc = self.loc();
                    nodes.push(Node::new_string(s.clone(), end_loc));
                    self.get()?;
                    return Ok(Node::new_interporated_string(
                        nodes,
                        start_loc.merge(end_loc),
                    ));
                }
                TokenKind::IntermediateDoubleQuote(s) => {
                    nodes.push(Node::new_string(s.clone(), self.loc()));
                    self.get()?;
                }
                TokenKind::OpenDoubleQuote(s) => {
                    let s = s.clone();
                    self.get()?;
                    self.parse_interporated_string_literal(&s)?;
                }
                TokenKind::EOF => {
                    return Err(self.error_unexpected(self.loc(), "Unexpectd EOF."));
                }
                _ => {
                    nodes.push(self.parse_comp_stmt()?);
                }
            }
        }
    }

    fn parse_hash_literal(&mut self) -> Result<Node, RubyError> {
        let mut kvp = vec![];
        let loc = self.prev_loc();
        loop {
            if self.consume_punct(Punct::RBrace) {
                return Ok(Node::new_hash(kvp, loc.merge(self.prev_loc())));
            };
            let ident_loc = self.loc();
            let mut symbol_flag = false;
            let key = if self.peek().can_be_symbol() {
                self.save_state();
                let token = self.get()?.clone();
                let ident = self.token_as_symbol(&token);
                if self.consume_punct(Punct::Colon) {
                    self.discard_state();
                    let id = self.get_ident_id(ident);
                    symbol_flag = true;
                    Node::new_symbol(id, ident_loc)
                } else {
                    self.restore_state();
                    self.parse_arg()?
                }
            } else {
                self.parse_arg()?
            };
            if !symbol_flag {
                self.expect_punct(Punct::FatArrow)?
            };
            let value = self.parse_arg()?;
            kvp.push((key, value));
            if !self.consume_punct(Punct::Comma) {
                break;
            };
        }
        self.expect_punct(Punct::RBrace)?;
        Ok(Node::new_hash(kvp, loc.merge(self.prev_loc())))
    }

    fn parse_if_then(&mut self) -> Result<Node, RubyError> {
        //  if EXPR THEN
        //      COMPSTMT
        //      (elsif EXPR THEN COMPSTMT)*
        //      [else COMPSTMT]
        //  end
        let loc = self.prev_loc();
        let cond = self.parse_expr()?;
        self.parse_then()?;
        let then_ = self.parse_comp_stmt()?;
        let else_ = if self.consume_reserved(Reserved::Elsif) {
            self.parse_if_then()?
        } else if self.consume_reserved(Reserved::Else) {
            self.parse_comp_stmt()?
        } else {
            Node::new_comp_stmt(vec![], self.loc())
        };
        Ok(Node::new_if(cond, then_, else_, loc))
    }

    fn parse_unless(&mut self) -> Result<Node, RubyError> {
        //  unless EXPR THEN
        //      COMPSTMT
        //      [else COMPSTMT]
        //  end
        let loc = self.prev_loc();
        let cond = self.parse_expr()?;
        self.parse_then()?;
        let then_ = self.parse_comp_stmt()?;
        let else_ = if self.consume_reserved(Reserved::Else) {
            self.parse_comp_stmt()?
        } else {
            Node::new_comp_stmt(vec![], self.loc())
        };
        Ok(Node::new_if(cond, else_, then_, loc))
    }

    fn parse_then(&mut self) -> Result<(), RubyError> {
        if self.consume_term() {
            self.consume_reserved(Reserved::Then);
            return Ok(());
        }
        self.expect_reserved(Reserved::Then)?;
        Ok(())
    }

    fn parse_do(&mut self) -> Result<(), RubyError> {
        if self.consume_term() {
            //self.consume_reserved(Reserved::Do);
            return Ok(());
        }
        self.expect_reserved(Reserved::Do)?;
        Ok(())
    }

    fn parse_def(&mut self) -> Result<Node, RubyError> {
        //  def FNAME ARGDECL
        //      COMPSTMT
        //      [rescue [ARGS] [`=>' LHS] THEN COMPSTMT]+
        //      [else COMPSTMT]
        //      [ensure COMPSTMT]
        //  end
        let mut is_class_method = false;
        let tok = self.get()?.clone();
        let id = match tok.kind {
            TokenKind::Reserved(Reserved::Self_) => {
                is_class_method = true;
                self.expect_punct(Punct::Dot)?;
                self.expect_ident()?
            }
            TokenKind::Ident(name, has_suffix) => {
                if has_suffix {
                    match self.get()?.kind {
                        TokenKind::Punct(Punct::Question) => self.get_ident_id(name + "?"),
                        TokenKind::Punct(Punct::Not) => self.get_ident_id(name + "!"),
                        _ => return Err(self.error_unexpected(tok.loc, "Illegal method name.")),
                    }
                } else {
                    match self.peek_no_term().kind {
                        TokenKind::Punct(Punct::Assign) => {
                            self.get()?;
                            self.get_ident_id(name + "=")
                        }
                        _ => self.get_ident_id(name),
                    }
                }
            }
            TokenKind::Punct(Punct::Plus) => self.get_ident_id("+"),
            TokenKind::Punct(Punct::Minus) => self.get_ident_id("-"),
            TokenKind::Punct(Punct::Mul) => self.get_ident_id("*"),
            _ => return Err(self.error_unexpected(self.loc(), "Expected identifier or operator.")),
        };
        self.context_stack.push(Context::new_method());
        let args = self.parse_params()?;
        let body = self.parse_comp_stmt()?;
        self.expect_reserved(Reserved::End)?;
        let lvar = self.context_stack.pop().unwrap().lvar;
        //#[cfg(feature = "verbose")]
        //eprintln!("Parsed def name:{}", self.ident_table.get_name(id));
        if is_class_method {
            Ok(Node::new_class_method_decl(id, args, body, lvar))
        } else {
            Ok(Node::new_method_decl(id, args, body, lvar))
        }
    }

    // ( )
    // ( ident [, ident]* )
    fn parse_params(&mut self) -> Result<Vec<Node>, RubyError> {
        if self.consume_term() {
            return Ok(vec![]);
        };
        let paren_flag = self.consume_punct(Punct::LParen);
        let mut args = vec![];
        if paren_flag && self.consume_punct(Punct::RParen) {
            if !self.consume_term() {
                return Err(self.error_unexpected(self.loc(), "Expect terminator"));
            }
            return Ok(args);
        }
        #[allow(dead_code)]
        #[derive(Debug, Clone, PartialEq, PartialOrd)]
        enum Kind {
            Reqired,
            Optional,
            Rest,
            PostReq,
            KeyWord,
            KWRest,
        }
        let mut state = Kind::Reqired;
        loop {
            let mut loc = self.loc();
            if self.consume_punct(Punct::BitAnd) {
                // Block param
                let id = self.expect_ident()?;
                loc = loc.merge(self.prev_loc());
                args.push(Node::new_block_param(id, loc));
                self.new_block_param(id, loc)?;
                break;
            } else if self.consume_punct(Punct::Mul) {
                // Splat(Rest) param
                let id = self.expect_ident()?;
                loc = loc.merge(self.prev_loc());
                if state >= Kind::Rest {
                    return Err(self
                        .error_unexpected(loc, "Splat parameter is not allowed in ths position."));
                } else {
                    state = Kind::Rest;
                }

                args.push(Node::new_splat_param(id, loc));
                self.new_param(id, self.prev_loc())?;
            } else {
                let id = self.expect_ident()?;
                if self.consume_punct(Punct::Assign) {
                    // Optional param
                    let default = self.parse_arg()?;
                    loc = loc.merge(self.prev_loc());
                    match state {
                        Kind::Reqired => state = Kind::Optional,
                        Kind::Optional => {}
                        _ => {
                            return Err(self.error_unexpected(
                                loc,
                                "Optional parameter is not allowed in ths position.",
                            ))
                        }
                    };
                    args.push(Node::new_optional_param(id, default, loc));
                    self.new_param(id, loc)?;
                } else if self.consume_punct_no_term(Punct::Colon) {
                    // Keyword param
                    let default = if self.peek_no_term().kind == TokenKind::Punct(Punct::Comma) {
                        None
                    } else {
                        Some(self.parse_arg()?)
                    };
                    loc = loc.merge(self.prev_loc());
                    if state == Kind::KWRest {
                        return Err(self.error_unexpected(
                            loc,
                            "Keyword parameter is not allowed in ths position.",
                        ));
                    } else {
                        state = Kind::KeyWord;
                    };
                    args.push(Node::new_keyword_param(id, default, loc));
                    self.new_param(id, loc)?;
                } else {
                    // Required param
                    loc = self.prev_loc();
                    match state {
                        Kind::Reqired => {
                            args.push(Node::new_param(id, loc));
                            self.new_param(id, loc)?;
                        }
                        Kind::PostReq | Kind::Optional | Kind::Rest => {
                            args.push(Node::new_post_param(id, loc));
                            self.new_param(id, loc)?;
                            state = Kind::PostReq;
                        }
                        _ => {
                            return Err(self.error_unexpected(
                                loc,
                                "Required parameter is not allowed in ths position.",
                            ))
                        }
                    }
                };
            }
            if !self.consume_punct_no_term(Punct::Comma) {
                break;
            }
        }
        if paren_flag {
            self.expect_punct(Punct::RParen)?
        };
        if !self.consume_term() {
            return Err(self.error_unexpected(self.loc(), "Expect terminator."));
        }
        Ok(args)
    }

    fn parse_class(&mut self, is_module: bool) -> Result<Node, RubyError> {
        //  class identifier [`<' EXPR]
        //      COMPSTMT
        //  end
        let loc = self.loc();
        let name = match &self.get()?.kind {
            TokenKind::Const(s) => s.clone(),
            _ => return Err(self.error_unexpected(loc, "Class/Module name must be CONSTANT.")),
        };
        let superclass = if self.consume_punct_no_term(Punct::Lt) {
            if is_module {
                return Err(self.error_unexpected(self.prev_loc(), "Unexpected '<'."));
            };
            self.parse_expr()?
        } else {
            Node::new_const(IdentId::from(0usize), false, loc)
        };
        self.consume_term();
        let id = self.get_ident_id(&name);
        self.context_stack.push(Context::new_class(None));
        let body = self.parse_comp_stmt()?;
        self.expect_reserved(Reserved::End)?;
        let lvar = self.context_stack.pop().unwrap().lvar;
        #[cfg(feature = "verbose")]
        eprintln!(
            "Parsed {} name:{}",
            if is_module { "module" } else { "class" },
            name
        );
        Ok(Node::new_class_decl(
            id, superclass, body, lvar, is_module, loc,
        ))
    }

    fn parse_op_definable(&mut self, punct: &Punct) -> Result<IdentId, RubyError> {
        match punct {
            Punct::LBracket => {
                if self.consume_punct_no_term(Punct::RBracket) {
                    if self.consume_punct_no_term(Punct::Assign) {
                        Ok(self.get_ident_id("[]="))
                    } else {
                        Ok(self.get_ident_id("[]"))
                    }
                } else {
                    Err(self.error_unexpected(self.loc(), "Invalid symbol literal."))
                }
            }
            _ => Err(self.error_unexpected(self.prev_loc(), "Invalid symbol literal.")),
        }
    }
}
