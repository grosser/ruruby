pub struct Inst;
impl Inst {
    pub const PUSH_FIXNUM: u8 = 1;
    pub const PUSH_FLONUM: u8 = 2;
    pub const PUSH_TRUE: u8 = 3;
    pub const PUSH_FALSE: u8 = 4;
    pub const PUSH_NIL: u8 = 5;
    pub const PUSH_STRING: u8 = 6;
    pub const PUSH_SYMBOL: u8 = 7;
    pub const PUSH_SELF: u8 = 8;

    pub const ADD: u8 = 10;
    pub const SUB: u8 = 11;
    pub const MUL: u8 = 12;
    pub const DIV: u8 = 13;
    pub const REM: u8 = 14;
    pub const EQ: u8 = 15;
    pub const NE: u8 = 16;
    pub const TEQ: u8 = 17;
    pub const GT: u8 = 18;
    pub const GE: u8 = 19;
    pub const NOT: u8 = 20;
    pub const SHR: u8 = 21;
    pub const SHL: u8 = 22;
    pub const BIT_OR: u8 = 23;
    pub const BIT_AND: u8 = 24;
    pub const BIT_XOR: u8 = 25;
    pub const BIT_NOT: u8 = 26;

    pub const ADDI: u8 = 27;
    pub const SUBI: u8 = 28;
    pub const POW: u8 = 29;

    pub const SET_LOCAL: u8 = 30;
    pub const GET_LOCAL: u8 = 31;
    pub const GET_CONST: u8 = 32;
    pub const SET_CONST: u8 = 33;
    pub const GET_CONST_TOP: u8 = 34;
    pub const GET_SCOPE: u8 = 35;

    pub const GET_INSTANCE_VAR: u8 = 36;
    pub const SET_INSTANCE_VAR: u8 = 37;
    pub const GET_GLOBAL_VAR: u8 = 90;
    pub const SET_GLOBAL_VAR: u8 = 91;
    pub const GET_ARRAY_ELEM: u8 = 38;
    pub const SET_ARRAY_ELEM: u8 = 39;

    pub const SEND: u8 = 40;
    pub const SEND_SELF: u8 = 41;

    pub const CHECK_LOCAL: u8 = 42;

    pub const CREATE_RANGE: u8 = 50;
    pub const CREATE_ARRAY: u8 = 51;
    pub const CREATE_PROC: u8 = 52;
    pub const CREATE_HASH: u8 = 53;
    pub const CREATE_REGEXP: u8 = 54;

    pub const POP: u8 = 60;
    pub const DUP: u8 = 63;
    pub const TAKE: u8 = 64;
    pub const SPLAT: u8 = 65;
    pub const CONCAT_STRING: u8 = 61;
    pub const TO_S: u8 = 62;

    pub const DEF_CLASS: u8 = 70;
    pub const DEF_METHOD: u8 = 71;
    pub const DEF_CLASS_METHOD: u8 = 72;

    pub const JMP: u8 = 80;
    pub const JMP_IF_FALSE: u8 = 81;
    pub const END: u8 = 0;
    pub const RETURN: u8 = 82;
    pub const OPT_CASE: u8 = 83;
}

#[allow(dead_code)]
impl Inst {
    pub fn inst_name(inst: u8) -> &'static str {
        match inst {
            Inst::PUSH_FIXNUM => "PUSH_FIXNUM",
            Inst::PUSH_FLONUM => "PUSH_FLONUM",
            Inst::PUSH_TRUE => "PUSH_TRUE",
            Inst::PUSH_FALSE => "PUSH_FALSE",
            Inst::PUSH_NIL => "PUSH_NIL",
            Inst::PUSH_STRING => "PUSH_STRING",
            Inst::PUSH_SYMBOL => "PUSH_SYMBOL",
            Inst::PUSH_SELF => "PUSH_SELF",

            Inst::ADD => "ADD",
            Inst::SUB => "SUB",
            Inst::MUL => "MUL",
            Inst::DIV => "DIV",
            Inst::REM => "REM",
            Inst::EQ => "EQ",
            Inst::NE => "NE",
            Inst::TEQ => "TEQ",
            Inst::GT => "GT",
            Inst::GE => "GE",
            Inst::NOT => "NOT",
            Inst::SHR => "SHR",
            Inst::SHL => "SHL",
            Inst::BIT_OR => "BIT_OR",
            Inst::BIT_AND => "BIT_AND",
            Inst::BIT_XOR => "BIT_XOR",
            Inst::BIT_NOT => "BIT_NOT",

            Inst::ADDI => "ADDI",
            Inst::SUBI => "SUBI",
            Inst::POW => "POW",

            Inst::SET_LOCAL => "SET_LOCAL",
            Inst::GET_LOCAL => "GET_LOCAL",
            Inst::GET_CONST => "GET_CONST",
            Inst::SET_CONST => "SET_CONST",
            Inst::GET_CONST_TOP => "GET_CONSTTOP",
            Inst::GET_SCOPE => "GET_SCOPE",

            Inst::GET_INSTANCE_VAR => "GET_INST_VAR",
            Inst::SET_INSTANCE_VAR => "SET_INST_VAR",
            Inst::GET_GLOBAL_VAR => "GET_GLBL_VAR",
            Inst::SET_GLOBAL_VAR => "SET_GLBL_VAR",
            Inst::GET_ARRAY_ELEM => "GET_ARY_ELEM",
            Inst::SET_ARRAY_ELEM => "SET_ARY_ELEM",

            Inst::SEND => "SEND",
            Inst::SEND_SELF => "SEND_SELF",

            Inst::CHECK_LOCAL => "CHECK_LOCAL",

            Inst::CREATE_RANGE => "CREATE_RANGE",
            Inst::CREATE_ARRAY => "CREATE_ARRAY",
            Inst::CREATE_PROC => "CREATE_PROC",
            Inst::CREATE_HASH => "CREATE_HASH",
            Inst::CREATE_REGEXP => "CREATE_REGEX",

            Inst::POP => "POP",
            Inst::DUP => "DUP",
            Inst::TAKE => "TAKE",
            Inst::SPLAT => "SPLAT",
            Inst::CONCAT_STRING => "CONCAT_STR",
            Inst::TO_S => "TO_S",

            Inst::DEF_CLASS => "DEF_CLASS",
            Inst::DEF_METHOD => "DEF_METHOD",
            Inst::DEF_CLASS_METHOD => "DEF_CMETHOD",

            Inst::JMP => "JMP",
            Inst::JMP_IF_FALSE => "JMP_IF_FALSE",
            Inst::END => "END",
            Inst::RETURN => "RETURN",
            Inst::OPT_CASE => "OPT_CASE",

            _ => "undefined",
        }
    }

    pub fn inst_size(inst: u8) -> usize {
        match inst {
            Inst::END
            | Inst::PUSH_NIL
            | Inst::PUSH_TRUE
            | Inst::PUSH_FALSE
            | Inst::PUSH_SELF
            | Inst::SUB
            | Inst::DIV
            | Inst::REM
            | Inst::POW
            | Inst::EQ
            | Inst::NE
            | Inst::GT
            | Inst::GE
            | Inst::NOT
            | Inst::SHR
            | Inst::SHL
            | Inst::BIT_OR
            | Inst::BIT_AND
            | Inst::BIT_XOR
            | Inst::BIT_NOT
            | Inst::CONCAT_STRING
            | Inst::CREATE_RANGE
            | Inst::CREATE_REGEXP
            | Inst::TO_S
            | Inst::SPLAT
            | Inst::POP
            | Inst::RETURN => 1,

            Inst::PUSH_STRING
            | Inst::PUSH_SYMBOL
            | Inst::GET_CONST
            | Inst::SET_CONST
            | Inst::GET_CONST_TOP
            | Inst::GET_SCOPE
            | Inst::GET_INSTANCE_VAR
            | Inst::SET_INSTANCE_VAR
            | Inst::GET_GLOBAL_VAR
            | Inst::SET_GLOBAL_VAR
            | Inst::GET_ARRAY_ELEM
            | Inst::SET_ARRAY_ELEM
            | Inst::CREATE_ARRAY
            | Inst::CREATE_PROC
            | Inst::JMP
            | Inst::JMP_IF_FALSE
            | Inst::DUP
            | Inst::TAKE
            | Inst::ADD
            | Inst::MUL
            | Inst::ADDI
            | Inst::SUBI
            | Inst::CREATE_HASH => 5,

            Inst::PUSH_FIXNUM
            | Inst::PUSH_FLONUM
            | Inst::SET_LOCAL
            | Inst::GET_LOCAL
            | Inst::DEF_METHOD
            | Inst::DEF_CLASS_METHOD => 9,
            Inst::DEF_CLASS => 10,
            Inst::OPT_CASE => 13,
            Inst::SEND | Inst::SEND_SELF => 21,
            _ => 1,
        }
    }
}
