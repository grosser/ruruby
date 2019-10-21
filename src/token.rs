use crate::util::*;

pub type Token = Annot<TokenKind>;

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            TokenKind::EOF => write!(f, "Token![{:?}, {}],", self.kind, self.loc.0),
            TokenKind::Punct(punct) => write!(
                f,
                "Token![Punct(Punct::{:?}), {}, {}],",
                punct, self.loc.0, self.loc.1
            ),
            TokenKind::Reserved(reserved) => write!(
                f,
                "Token![Reserved(Reserved::{:?}), {}, {}],",
                reserved, self.loc.0, self.loc.1
            ),
            _ => write!(
                f,
                "Token![{:?}, {}, {}],",
                self.kind, self.loc.0, self.loc.1
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Nop,
    EOF,
    Ident(String),
    Const(String),
    NumLit(i64),
    StringLit(String),
    Reserved(Reserved),
    Punct(Punct),
    Space,
    LineTerm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Reserved {
    BEGIN,
    END,
    Alias,
    Begin,
    Break,
    Case,
    Class,
    Def,
    Defined,
    Do,
    Else,
    Elsif,
    End,
    False,
    If,
    Return,
    Then,
    True,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Punct {
    LParen,
    RParen,
    Semi,
    Colon,
    Comma,
    Dot,

    Plus,
    Minus,
    Mul,
    And,
    Or,
    Assign,
    Eq,
    Ne,
    Gt,
    Ge,
    Lt,
    Le,
    LAnd,
    LOr,
}

#[allow(unused)]
impl Token {
    pub fn new_ident(ident: impl Into<String>, loc: Loc) -> Self {
        Annot::new(TokenKind::Ident(ident.into()), loc)
    }

    pub fn new_const(ident: impl Into<String>, loc: Loc) -> Self {
        Annot::new(TokenKind::Const(ident.into()), loc)
    }

    pub fn new_reserved(ident: Reserved, loc: Loc) -> Self {
        Annot::new(TokenKind::Reserved(ident), loc)
    }

    pub fn new_numlit(num: i64, loc: Loc) -> Self {
        Annot::new(TokenKind::NumLit(num), loc)
    }

    pub fn new_stringlit(string: String, loc: Loc) -> Self {
        Annot::new(TokenKind::StringLit(string), loc)
    }

    pub fn new_punct(punct: Punct, loc: Loc) -> Self {
        Annot::new(TokenKind::Punct(punct), loc)
    }

    pub fn new_space(loc: Loc) -> Self {
        Annot::new(TokenKind::Space, loc)
    }

    pub fn new_line_term(loc: Loc) -> Self {
        Annot::new(TokenKind::LineTerm, loc)
    }
    pub fn new_eof(pos: usize) -> Self {
        Annot::new(TokenKind::EOF, Loc(pos, pos))
    }
}

impl Token {
    /// Examine the token, and return true if it is a line terminator.
    pub fn is_line_term(&self) -> bool {
        self.kind == TokenKind::LineTerm
    }

    /// Examine the token, and return true if it is EOF.
    pub fn is_eof(&self) -> bool {
        self.kind == TokenKind::EOF
    }

    /// Examine the token, and return true if it is a line terminator or ';' or EOF.
    pub fn is_term(&self) -> bool {
        match self.kind {
            TokenKind::LineTerm | TokenKind::EOF | TokenKind::Punct(Punct::Semi) => true,
            _ => false,
        }
    }
}