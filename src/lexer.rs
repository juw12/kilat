//! Lexer: source text -> tokens.

use crate::types::{Diag, Pos};

#[derive(Clone, Debug, PartialEq)]
pub enum StrPart {
    Lit(String),
    /// Raw source of an interpolated `{...}` and where it starts.
    Code(String, Pos),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    Int(i64),
    Float(f64),
    Str(Vec<StrPart>),
    Ident(String),
    KwFn,
    KwLet,
    KwVar,
    KwIf,
    KwElse,
    KwWhile,
    KwFor,
    KwIn,
    KwMatch,
    KwReturn,
    KwBreak,
    KwContinue,
    KwStruct,
    KwEnum,
    KwTrue,
    KwFalse,
    KwAnd,
    KwOr,
    KwNot,
    KwDiv,
    LParen,
    RParen,
    LBrack,
    RBrack,
    LBrace,
    RBrace,
    Comma,
    Colon,
    Dot,
    DotDot,
    DotDotEq,
    Arrow,
    FatArrow,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Caret,
    Assign,
    EqEq,
    NotEq,
    Lt,
    Le,
    Gt,
    Ge,
    PlusEq,
    MinusEq,
    StarEq,
    SlashEq,
    Bang,
    Question,
    QQ,
    Pipe,
    PipePipe,
    Newline,
    Eof,
}

#[derive(Clone, Debug)]
pub struct Token {
    pub tok: Tok,
    pub pos: Pos,
    /// True when whitespace came before this token.
    pub space: bool,
}

struct Lexer {
    src: Vec<char>,
    i: usize,
    line: u32,
    col: u32,
    file: u16,
    internal: bool,
    out: Vec<Token>,
    nest: Vec<char>,
}

impl Lexer {
    fn pos(&self) -> Pos {
        Pos {
            line: self.line,
            col: self.col.min(65535) as u16,
            file: self.file,
        }
    }

    fn peek(&self, k: usize) -> char {
        *self.src.get(self.i + k).unwrap_or(&'\0')
    }

    fn eof(&self) -> bool {
        self.i >= self.src.len()
    }

    fn bump(&mut self) -> char {
        let c = self.src[self.i];
        self.i += 1;
        if c == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        c
    }

    fn newline(&mut self, pos: Pos) {
        match self.out.last() {
            None => {}
            Some(t) if t.tok == Tok::Newline => {}
            _ => self.out.push(Token {
                tok: Tok::Newline,
                pos,
                space: false,
            }),
        }
    }

    fn digits(&mut self, text: &mut String) {
        while self.peek(0).is_ascii_digit() || (self.peek(0) == '_' && self.peek(1).is_ascii_digit())
        {
            let c = self.bump();
            if c != '_' {
                text.push(c);
            }
        }
    }

    fn radix(&mut self, pos: Pos, radix: u32) -> Result<Tok, Diag> {
        self.bump();
        self.bump();
        let mut text = String::new();
        while self.peek(0).is_digit(radix) || self.peek(0) == '_' {
            let c = self.bump();
            if c != '_' {
                text.push(c);
            }
        }
        match i64::from_str_radix(&text, radix) {
            Ok(v) => Ok(Tok::Int(v)),
            Err(_) => Err(Diag::new(pos, "integer literal is too large")),
        }
    }

    fn number(&mut self, pos: Pos) -> Result<Tok, Diag> {
        let after_dot = matches!(self.out.last(), Some(t) if t.tok == Tok::Dot);
        if !after_dot && self.peek(0) == '0' {
            let p1 = self.peek(1);
            if (p1 == 'x' || p1 == 'X') && self.peek(2).is_ascii_hexdigit() {
                return self.radix(pos, 16);
            }
            if (p1 == 'b' || p1 == 'B') && (self.peek(2) == '0' || self.peek(2) == '1') {
                return self.radix(pos, 2);
            }
        }
        let mut text = String::new();
        self.digits(&mut text);
        let mut is_float = false;
        if !after_dot {
            if self.peek(0) == '.' && self.peek(1).is_ascii_digit() {
                is_float = true;
                text.push(self.bump());
                self.digits(&mut text);
            }
            if self.peek(0) == 'e' || self.peek(0) == 'E' {
                let k = if self.peek(1) == '+' || self.peek(1) == '-' {
                    2
                } else {
                    1
                };
                if self.peek(k).is_ascii_digit() {
                    is_float = true;
                    self.bump();
                    text.push('e');
                    if k == 2 {
                        text.push(self.bump());
                    }
                    self.digits(&mut text);
                }
            }
        }
        if is_float {
            match text.parse::<f64>() {
                Ok(v) => Ok(Tok::Float(v)),
                Err(_) => Err(Diag::new(pos, "invalid number")),
            }
        } else {
            match text.parse::<i64>() {
                Ok(v) => Ok(Tok::Int(v)),
                Err(_) => Err(Diag::new(pos, "integer literal is too large")
                    .with_hint("the largest Int is 9223372036854775807; use a Float like 1e19")),
            }
        }
    }

    fn escape(&mut self, pos: Pos, cur: &mut String) -> Result<(), Diag> {
        if self.eof() {
            return Err(Diag::new(pos, "unterminated string"));
        }
        let epos = self.pos();
        let c = self.bump();
        match c {
            'n' => cur.push('\n'),
            't' => cur.push('\t'),
            'r' => cur.push('\r'),
            '0' => cur.push('\0'),
            '\\' => cur.push('\\'),
            '"' => cur.push('"'),
            '\'' => cur.push('\''),
            '{' => cur.push('{'),
            '}' => cur.push('}'),
            'u' => {
                if self.peek(0) != '{' {
                    return Err(Diag::new(epos, "expected `{` after `\\u`")
                        .with_hint("write a unicode escape as `\\u{1F600}`"));
                }
                self.bump();
                let mut hex = String::new();
                while !self.eof() && self.peek(0) != '}' && hex.len() < 8 {
                    hex.push(self.bump());
                }
                if self.peek(0) != '}' {
                    return Err(Diag::new(epos, "unterminated `\\u{...}` escape"));
                }
                self.bump();
                let ch = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32);
                match ch {
                    Some(ch) => cur.push(ch),
                    None => return Err(Diag::new(epos, "invalid unicode escape")),
                }
            }
            other => {
                return Err(Diag::new(epos, format!("unknown escape `\\{}`", other))
                    .with_hint("valid escapes: \\n \\t \\r \\0 \\\\ \\\" \\' \\{ \\} \\u{...}"))
            }
        }
        Ok(())
    }

    /// Capture the raw source of `{...}` inside a string, up to the matching `}`.
    fn interp_code(&mut self, start: Pos) -> Result<String, Diag> {
        let mut code = String::new();
        let mut depth = 0i32;
        loop {
            if self.eof() {
                return Err(Diag::new(start, "unterminated `{` inside string")
                    .with_hint("close it with `}`, or write `{{` for a literal brace"));
            }
            let c = self.peek(0);
            match c {
                '}' if depth == 0 => {
                    self.bump();
                    break;
                }
                '{' | '(' | '[' => depth += 1,
                '}' | ')' | ']' => depth -= 1,
                '"' | '\'' => {
                    let q = self.bump();
                    code.push(q);
                    loop {
                        if self.eof() {
                            return Err(Diag::new(start, "unterminated string inside `{...}`"));
                        }
                        let d = self.bump();
                        code.push(d);
                        if d == '\\' {
                            if !self.eof() {
                                code.push(self.bump());
                            }
                        } else if d == q {
                            break;
                        }
                    }
                    continue;
                }
                _ => {}
            }
            code.push(self.bump());
        }
        Ok(code)
    }

    fn string(&mut self, pos: Pos) -> Result<Tok, Diag> {
        let quote = self.bump();
        let interp = quote == '"';
        let mut parts: Vec<StrPart> = Vec::new();
        let mut cur = String::new();
        loop {
            if self.eof() {
                return Err(Diag::new(pos, "unterminated string").with_hint("add a closing quote"));
            }
            let c = self.bump();
            if c == quote {
                break;
            }
            match c {
                '\\' => self.escape(pos, &mut cur)?,
                '{' if interp => {
                    if self.peek(0) == '{' {
                        self.bump();
                        cur.push('{');
                    } else {
                        let cpos = self.pos();
                        let code = self.interp_code(cpos)?;
                        if !cur.is_empty() {
                            parts.push(StrPart::Lit(std::mem::take(&mut cur)));
                        }
                        parts.push(StrPart::Code(code, cpos));
                    }
                }
                '}' if interp => {
                    if self.peek(0) == '}' {
                        self.bump();
                    }
                    cur.push('}');
                }
                _ => cur.push(c),
            }
        }
        if !cur.is_empty() || parts.is_empty() {
            parts.push(StrPart::Lit(cur));
        }
        Ok(Tok::Str(parts))
    }

    fn word(&mut self) -> Tok {
        let mut s = String::new();
        loop {
            let c = self.peek(0);
            if self.eof() {
                break;
            }
            if c.is_alphanumeric() || c == '_' || (self.internal && c == '$') {
                s.push(self.bump());
            } else {
                break;
            }
        }
        match s.as_str() {
            "fn" => Tok::KwFn,
            "let" => Tok::KwLet,
            "var" => Tok::KwVar,
            "if" => Tok::KwIf,
            "else" => Tok::KwElse,
            "while" => Tok::KwWhile,
            "for" => Tok::KwFor,
            "in" => Tok::KwIn,
            "match" => Tok::KwMatch,
            "return" => Tok::KwReturn,
            "break" => Tok::KwBreak,
            "continue" => Tok::KwContinue,
            "struct" => Tok::KwStruct,
            "enum" => Tok::KwEnum,
            "true" => Tok::KwTrue,
            "false" => Tok::KwFalse,
            "and" => Tok::KwAnd,
            "or" => Tok::KwOr,
            "not" => Tok::KwNot,
            "div" => Tok::KwDiv,
            _ => Tok::Ident(s),
        }
    }

    fn operator(&mut self, pos: Pos) -> Result<Tok, Diag> {
        let c = self.bump();
        let n = self.peek(0);
        let tok = match c {
            '(' => {
                self.nest.push('(');
                Tok::LParen
            }
            '[' => {
                self.nest.push('[');
                Tok::LBrack
            }
            '{' => {
                self.nest.push('{');
                Tok::LBrace
            }
            ')' => {
                self.nest.pop();
                Tok::RParen
            }
            ']' => {
                self.nest.pop();
                Tok::RBrack
            }
            '}' => {
                self.nest.pop();
                Tok::RBrace
            }
            ',' => Tok::Comma,
            ':' => Tok::Colon,
            '.' => {
                if n == '.' {
                    self.bump();
                    if self.peek(0) == '=' {
                        self.bump();
                        Tok::DotDotEq
                    } else {
                        Tok::DotDot
                    }
                } else {
                    Tok::Dot
                }
            }
            '-' => {
                if n == '>' {
                    self.bump();
                    Tok::Arrow
                } else if n == '=' {
                    self.bump();
                    Tok::MinusEq
                } else {
                    Tok::Minus
                }
            }
            '=' => {
                if n == '>' {
                    self.bump();
                    Tok::FatArrow
                } else if n == '=' {
                    self.bump();
                    Tok::EqEq
                } else {
                    Tok::Assign
                }
            }
            '+' => {
                if n == '=' {
                    self.bump();
                    Tok::PlusEq
                } else {
                    Tok::Plus
                }
            }
            '*' => {
                if n == '=' {
                    self.bump();
                    Tok::StarEq
                } else if n == '*' {
                    return Err(Diag::new(pos, "`**` is not an operator")
                        .with_hint("use `^` for powers: `x ^ 2`"));
                } else {
                    Tok::Star
                }
            }
            '/' => {
                if n == '=' {
                    self.bump();
                    Tok::SlashEq
                } else {
                    Tok::Slash
                }
            }
            '%' => Tok::Percent,
            '^' => Tok::Caret,
            '!' => {
                if n == '=' {
                    self.bump();
                    Tok::NotEq
                } else {
                    Tok::Bang
                }
            }
            '<' => {
                if n == '=' {
                    self.bump();
                    Tok::Le
                } else {
                    Tok::Lt
                }
            }
            '>' => {
                if n == '=' {
                    self.bump();
                    Tok::Ge
                } else {
                    Tok::Gt
                }
            }
            '?' => {
                if n == '?' {
                    self.bump();
                    Tok::QQ
                } else {
                    Tok::Question
                }
            }
            '|' => {
                if n == '|' {
                    self.bump();
                    Tok::PipePipe
                } else {
                    Tok::Pipe
                }
            }
            '&' => {
                if n == '&' {
                    self.bump();
                    Tok::KwAnd
                } else {
                    return Err(Diag::new(pos, "unexpected `&`").with_hint("use `and`"));
                }
            }
            other => {
                return Err(Diag::new(pos, format!("unexpected character `{}`", other)));
            }
        };
        Ok(tok)
    }
}

/// Turn source text into tokens. `line0`/`col0` give the position of the first
/// character (used when lexing code inside string interpolation).
pub fn lex(src: &str, file: u16, line0: u32, col0: u32, internal: bool) -> Result<Vec<Token>, Diag> {
    let mut lx = Lexer {
        src: src.chars().collect(),
        i: 0,
        line: line0,
        col: col0,
        file,
        internal,
        out: Vec::new(),
        nest: Vec::new(),
    };
    let mut space = true;
    while !lx.eof() {
        let c = lx.peek(0);
        let pos = lx.pos();
        if c == ' ' || c == '\t' || c == '\r' || c == '\u{feff}' {
            lx.bump();
            space = true;
            continue;
        }
        if c == '\n' {
            lx.bump();
            space = true;
            let suppressed = matches!(lx.nest.last(), Some('(') | Some('['));
            if !suppressed {
                lx.newline(pos);
            }
            continue;
        }
        if c == ';' {
            lx.bump();
            lx.newline(pos);
            space = true;
            continue;
        }
        if c == '/' && lx.peek(1) == '/' {
            while !lx.eof() && lx.peek(0) != '\n' {
                lx.bump();
            }
            continue;
        }
        if c == '/' && lx.peek(1) == '*' {
            lx.bump();
            lx.bump();
            let mut depth = 1;
            while depth > 0 {
                if lx.eof() {
                    return Err(Diag::new(pos, "unterminated `/*` comment").with_hint("close it with `*/`"));
                }
                if lx.peek(0) == '/' && lx.peek(1) == '*' {
                    lx.bump();
                    lx.bump();
                    depth += 1;
                } else if lx.peek(0) == '*' && lx.peek(1) == '/' {
                    lx.bump();
                    lx.bump();
                    depth -= 1;
                } else {
                    lx.bump();
                }
            }
            space = true;
            continue;
        }
        let tok = if c.is_ascii_digit() {
            lx.number(pos)?
        } else if c == '"' || c == '\'' {
            lx.string(pos)?
        } else if c.is_alphabetic() || c == '_' || (c == '$' && internal) {
            lx.word()
        } else {
            lx.operator(pos)?
        };
        lx.out.push(Token { tok, pos, space });
        space = false;
    }
    let pos = lx.pos();
    lx.newline(pos);
    lx.out.push(Token {
        tok: Tok::Eof,
        pos,
        space: true,
    });
    Ok(lx.out)
}
