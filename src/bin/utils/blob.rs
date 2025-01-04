use std::{
    collections::HashMap,
    fs::{self},
};

/// Represents a file after being parsed
pub struct KartaFile {
    atoms: HashMap<String, usize>,
    root: AstId,
    ast_heap: AstHeap,
}

impl KartaFile {
    pub fn new(text: String) -> Result<Self, String> {
        let mut atoms = HashMap::new();
        put_atoms_in_set(&mut atoms, String::from(".nil"));
        put_atoms_in_set(&mut atoms, String::from(".t"));
        put_atoms_in_set(&mut atoms, String::from(".head"));
        put_atoms_in_set(&mut atoms, String::from(".tail"));

        let mut ast_heap = AstHeap::new();

        let mut parser = Parser::new();
        let root = parser.parse(text, &mut ast_heap, &mut atoms)?;

        Ok(Self {
            ast_heap,
            atoms,
            root,
        })
    }

    pub fn from_file(filename: &'static str) -> Result<Self, String> {
        let mut text: String = match fs::read_to_string(filename) {
            Ok(c) => c,
            Err(x) => return Err(x.to_string()),
        };
        text.push('\n'); // This is required to make the tokenizer happy
        Self::new(text)
    }
}

fn put_atoms_in_set(atoms: &mut HashMap<String, usize>, atom: String) -> usize {
    if let Some(the_atom) = atoms.get(&atom) {
        return *the_atom;
    } else {
        let the_atom = atoms.len();
        atoms.insert(atom, the_atom);
        the_atom
    }
}

struct AstHeap {
    asts: Vec<Ast>,
}

impl AstHeap {
    fn new() -> Self {
        Self { asts: vec![] }
    }

    fn insert(&mut self, ast: Ast) -> AstId {
        let retval = AstId::new(self.asts.len());
        self.asts.push(ast);
        retval
    }

    fn create_int(&mut self, value: i64) -> AstId {
        self.insert(Ast::Int(value))
    }

    fn create_float(&mut self, value: f64) -> AstId {
        self.insert(Ast::Float(value))
    }

    fn create_char(&mut self, value: u8) -> AstId {
        self.insert(Ast::Char(value))
    }

    fn create_string(&mut self, value: String) -> AstId {
        self.insert(Ast::String(value))
    }

    fn create_atom(&mut self, value: usize) -> AstId {
        self.insert(Ast::Atom(value))
    }

    fn create_map(&mut self, value: HashMap<usize, AstId>) -> AstId {
        self.insert(Ast::Map(value))
    }

    fn make_nil_atom(&mut self, atoms: &mut HashMap<String, usize>) -> AstId {
        self.create_atom(put_atoms_in_set(atoms, String::from(".nil")))
    }

    fn make_list_node(
        &mut self,
        head_atom: usize,
        head: AstId,
        tail_atom: usize,
        atoms: &mut HashMap<String, usize>,
    ) -> AstId {
        let mut fields: HashMap<usize, AstId> = HashMap::new();
        fields.insert(head_atom, head);
        fields.insert(tail_atom, self.make_nil_atom(atoms));
        self.create_map(fields)
    }

    fn get(&mut self, id: AstId) -> Option<&mut Ast> {
        self.asts.get_mut(id.as_usize())
    }
}

#[derive(Copy, Clone, Debug)]
/// Unique identifier of an Ast expression in the file's vector of Asts
struct AstId(usize);

impl AstId {
    fn new(id: usize) -> Self {
        AstId(id)
    }

    fn as_usize(&self) -> usize {
        self.0
    }
}

/// Represents an expression
enum Ast {
    /// A basic integer
    Int(i64),
    /// A floating point number
    Float(f64),
    /// A character
    Char(u8),
    /// A string
    String(String),
    /// An atomic value
    Atom(usize),
    /// Maps an atom to an Ast within the file
    Map(HashMap<usize, AstId>),
}

#[derive(PartialEq, Clone, Copy, Debug)]
/// Represents the various kinds a token can be
enum TokenKind {
    LeftBrace,
    RightBrace,
    LeftSquare,
    RightSquare,
    Atom,
    Integer,
    Float,
    Char,
    String,
    Identifier,
    Comma,
    Assign,
    EndOfFile,
}

impl TokenKind {
    fn from_string(str: &str) -> Self {
        assert!(str.len() > 0);
        match str {
            "{" => TokenKind::LeftBrace,
            "}" => TokenKind::RightBrace,
            "[" => TokenKind::LeftSquare,
            "]" => TokenKind::RightSquare,
            "," => TokenKind::Comma,
            "=" => TokenKind::Assign,
            _ if str.chars().nth(0).unwrap() == '.' => TokenKind::Atom,
            _ if str.chars().nth(0).unwrap().is_digit(10) => TokenKind::Integer,
            _ => TokenKind::Identifier,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Span {
    line: usize,
    col: usize,
}

#[derive(Clone, Debug)]
struct Token {
    data: String,
    kind: TokenKind,
    span: Span,
}

struct Tokenizer {
    cursor: usize,
    text: String,
    line: usize,
    col: usize,
    state: State,
    starting_cursor: usize,
}

impl Tokenizer {
    fn new(text: String) -> Self {
        Self {
            cursor: 0,
            text,
            line: 1,
            col: 1,
            state: State::None,
            starting_cursor: 0,
        }
    }

    fn tokenize(&mut self, tokens: &mut Vec<Token>) -> Result<(), String> {
        while !self.eof() {
            let char = self.text.chars().nth(self.cursor).unwrap(); // yeah probably slow, but it doesn't matter

            match self.state {
                // The none state branches off into various other states depending on the next character
                State::None if char.is_whitespace() => self.advance(State::Whitespace),
                State::None if char.is_digit(10) => self.advance(State::Integer),
                State::None if char == '.' => self.advance(State::Atom),
                State::None if char == '\'' => self.advance(State::Char),
                State::None if char == '"' => self.advance(State::String),
                State::None if char == ';' => self.advance(State::Comment),
                State::None => self.advance(State::Symbol),

                // Whitespace state ends when the next char isn't whitespace
                State::Whitespace if self.eof() || !char.is_whitespace() => {
                    self.starting_cursor = self.cursor;
                    self.state = State::None
                }
                State::Whitespace => {
                    if char == '\n' {
                        self.line += 1;
                        self.col = 1;
                    }
                    self.advance(State::Whitespace);
                }

                // Integers become floats if a `.` is encountered, otherwise end when the next char isn't a digit
                State::Integer if char == '.' => self.advance(State::Float),
                State::Integer if self.eof() || !char.is_digit(10) => {
                    self.add_token(TokenKind::Integer, tokens);
                }
                State::Integer => self.advance(State::Integer),

                // Atoms end when the next char isn't a valid atom character
                State::Atom
                    if self.eof()
                        || (!char.is_alphanumeric()
                            && char != '_'
                            && char != '-'
                            && char != '?') =>
                {
                    self.add_token(TokenKind::Atom, tokens)
                }
                State::Atom => self.advance(State::Atom),

                // Strings end at the second single quote
                State::Char if self.eof() => {
                    return Err(String::from("error: char goes to end of file"))
                }
                State::Char if char == '\'' => {
                    self.advance(State::None);
                    self.add_token(TokenKind::Char, tokens);
                }
                State::Char => self.advance(State::Char),

                // Strings end at the second double quote
                State::String if self.eof() => {
                    return Err(String::from("error: string goes to end of file"))
                }
                State::String if char == '"' => {
                    self.advance(State::None);
                    self.add_token(TokenKind::String, tokens);
                }
                State::String => self.advance(State::String),

                // Symbols end at the end of the file, or if the next token isn't recognized
                State::Symbol
                    if self.eof()
                        || TokenKind::from_string(
                            &self.text[self.starting_cursor..self.cursor + 1],
                        ) == TokenKind::Identifier =>
                {
                    let token_data = &self.text[self.starting_cursor..self.cursor];
                    let token_kind = TokenKind::from_string(token_data);
                    self.add_token(token_kind, tokens);
                }
                State::Symbol => self.advance(State::Symbol),

                // Floats end at the end of file, or if the character is no longer a digit
                State::Float if self.eof() || !char.is_digit(10) => {
                    self.add_token(TokenKind::Float, tokens)
                }
                State::Float => self.advance(State::Float),

                // Comments end at newlines
                State::Comment if char == '\n' => {
                    self.line += 1;
                    self.col = 1;
                    self.starting_cursor = self.cursor;
                    self.state = State::None
                }
                State::Comment => self.advance(State::Comment),
            }
        }

        self.add_token(TokenKind::EndOfFile, tokens);

        Ok(())
    }

    fn eof(&self) -> bool {
        self.text.chars().nth(self.cursor).is_none()
    }

    fn advance(&mut self, new_state: State) {
        self.cursor += 1;
        self.col += 1;
        self.state = new_state;
    }

    fn add_token(&mut self, kind: TokenKind, tokens: &mut Vec<Token>) {
        let token_data = String::from(&self.text[self.starting_cursor..self.cursor]);
        let token = Token {
            data: token_data,
            kind,
            span: Span {
                line: self.col - 1,
                col: self.line,
            },
        };
        tokens.push(token);

        self.starting_cursor = self.cursor;
        self.state = State::None;
    }
}

enum State {
    None,
    Whitespace,
    Integer,
    Atom,
    Char,
    String,
    Symbol,
    Float,
    Comment,
}

struct Parser {
    cursor: usize,
    tokens: Vec<Token>,
}

impl Parser {
    fn new() -> Self {
        Self {
            cursor: 0,
            tokens: vec![],
        }
    }

    fn parse(
        &mut self,
        text: String,
        ast_heap: &mut AstHeap,
        atoms: &mut HashMap<String, usize>,
    ) -> Result<AstId, String> {
        let mut tokenizer = Tokenizer::new(text);
        let _ = tokenizer.tokenize(&mut self.tokens).unwrap();

        println!("{:?}", self.tokens);

        self.parse_expression(ast_heap, atoms)
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.cursor]
    }

    fn pop(&mut self) -> &Token {
        self.cursor += 1;
        &self.tokens[self.cursor - 1]
    }

    fn eos(&self) -> bool {
        self.cursor >= self.tokens.len()
    }

    fn parse_error(&self, expected: String, got: String) -> String {
        format!(
            "error: {}:{}: expected {}, got {}",
            self.peek().span.line,
            self.peek().span.col,
            expected,
            got
        )
    }

    fn accept(&mut self, kind: TokenKind) -> Option<&Token> {
        if !self.eos() && self.peek().kind == kind {
            Some(self.pop())
        } else {
            None
        }
    }

    fn expect(&mut self, kind: TokenKind) -> Result<&Token, String> {
        let peeked = self.peek().kind;
        let err = self.parse_error(format!("{:?}", kind), format!("{:?}", peeked));
        self.accept(kind).ok_or(err)
    }

    fn parse_expression(
        &mut self,
        ast_heap: &mut AstHeap,
        atoms: &mut HashMap<String, usize>,
    ) -> Result<AstId, String> {
        if let Some(token) = self.accept(TokenKind::Integer) {
            Ok(ast_heap.create_int(token.data.parse::<i64>().unwrap()))
        } else if let Some(token) = self.accept(TokenKind::Float) {
            Ok(ast_heap.create_float(token.data.parse::<f64>().unwrap()))
        } else if let Some(token) = self.accept(TokenKind::Char) {
            Ok(ast_heap.create_char(token.data.as_bytes()[1]))
        } else if let Some(token) = self.accept(TokenKind::String) {
            let token_len = token.data.len();
            Ok(ast_heap.create_string(String::from(&token.data[1..token_len - 1])))
        } else if let Some(token) = self.accept(TokenKind::Atom) {
            let atom_value = put_atoms_in_set(atoms, token.data.clone());
            Ok(ast_heap.create_atom(atom_value))
        } else if let Some(_token) = self.accept(TokenKind::LeftBrace) {
            let mut children: HashMap<usize, AstId> = HashMap::new();
            loop {
                let key_ast_id = self.parse_expression(ast_heap, atoms)?;
                let key = match *ast_heap.get(key_ast_id).unwrap() {
                    Ast::Atom(s) => s,
                    _ => return Err(String::from("bad!")),
                };
                let _ = self.expect(TokenKind::Assign)?;
                let value = self.parse_expression(ast_heap, atoms)?;

                children.insert(key, value);

                if self.accept(TokenKind::Comma).is_none() {
                    break;
                }
            }
            let _ = self.expect(TokenKind::RightBrace)?;

            Ok(ast_heap.create_map(children))
        } else if let Some(_token) = self.accept(TokenKind::LeftSquare) {
            let head_atom = put_atoms_in_set(atoms, String::from(".head"));
            let tail_atom = put_atoms_in_set(atoms, String::from(".tail"));

            if self.accept(TokenKind::RightSquare).is_some() {
                Ok(ast_heap.make_nil_atom(atoms))
            } else {
                let head = self.parse_expression(ast_heap, atoms)?;
                let retval = ast_heap.make_list_node(head_atom, head, tail_atom, atoms);
                let mut curr_id = retval;
                while self.accept(TokenKind::Comma).is_some() {
                    let head = self.parse_expression(ast_heap, atoms)?;
                    let new_map_id = ast_heap.make_list_node(head_atom, head, tail_atom, atoms);
                    let curr_map = if let Ast::Map(map) = ast_heap.get(curr_id).unwrap() {
                        map
                    } else {
                        panic!("unreachable")
                    };
                    curr_map.insert(tail_atom, new_map_id);
                    curr_id = new_map_id;
                }
                let _ = self.expect(TokenKind::RightSquare)?;
                Ok(retval)
            }
        } else {
            Err(self.parse_error(
                String::from("an expression"),
                format!("{:?}", self.peek().kind),
            ))
        }
    }
}
