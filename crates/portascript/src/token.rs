use crate::PsError;

/// Token types for the portascript lexer.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Eof,
    Newline,
    Comment(String),

    // Literals.
    StringLit(String),
    IntLit(i64),
    FloatLit(f64),

    // Identifiers and keywords.
    Ident(String),
    True,
    False,

    // Operators.
    Plus,
    Minus,
    Star,
    Slash,
    Percent,

    // Symbols.
    LParen,
    RParen,
    Comma,
    Eq,
}

/// Character-by-character tokenizer.
pub struct Tokenizer {
    chars: Vec<char>,
    pos: usize,
    pub line: usize,
    pub col: usize,
}

impl Tokenizer {
    pub fn new(source: &str) -> Self {
        Self {
            chars: source.chars().collect(),
            pos: 0,
            line: 1,
            col: 1,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.chars.get(self.pos).copied()?;
        self.pos += 1;
        if ch == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(ch)
    }

    fn skip_whitespace_no_newline(&mut self) {
        while let Some(ch) = self.peek() {
            if ch == ' ' || ch == '\t' || ch == '\r' {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn read_string(&mut self, quote: char) -> Result<String, PsError> {
        let (line, col) = (self.line, self.col);
        self.advance(); // consume opening quote
        let mut s = String::new();
        loop {
            match self.advance() {
                None => {
                    return Err(PsError {
                        message: "unterminated string".into(),
                        line,
                        col,
                    });
                }
                Some(ch) if ch == quote => return Ok(s),
                Some(ch) => s.push(ch),
            }
        }
    }

    fn read_number(&mut self) -> Token {
        let mut s = String::new();
        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() {
                s.push(ch);
                self.advance();
            } else {
                break;
            }
        }
        // Check for float.
        if self.peek() == Some('.') {
            // Peek ahead to see if the next char is a digit.
            if self.chars.get(self.pos + 1).map_or(false, |c| c.is_ascii_digit()) {
                s.push('.');
                self.advance(); // consume '.'
                while let Some(ch) = self.peek() {
                    if ch.is_ascii_digit() {
                        s.push(ch);
                        self.advance();
                    } else {
                        break;
                    }
                }
                return Token::FloatLit(s.parse().unwrap());
            }
        }
        Token::IntLit(s.parse().unwrap())
    }

    fn read_ident(&mut self) -> Token {
        let mut s = String::new();
        while let Some(ch) = self.peek() {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                s.push(ch);
                self.advance();
            } else {
                break;
            }
        }
        match s.as_str() {
            "true" => Token::True,
            "false" => Token::False,
            _ => Token::Ident(s),
        }
    }

    /// Read the next token.
    pub fn next_token(&mut self) -> Result<Token, PsError> {
        self.skip_whitespace_no_newline();

        match self.peek() {
            None => Ok(Token::Eof),
            Some('\n') => {
                self.advance();
                Ok(Token::Newline)
            }
            Some('#') => {
                let mut text = String::new();
                while let Some(ch) = self.peek() {
                    if ch == '\n' {
                        break;
                    }
                    text.push(ch);
                    self.advance();
                }
                Ok(Token::Comment(text))
            }
            Some('"') => {
                let s = self.read_string('"')?;
                Ok(Token::StringLit(s))
            }
            Some('\'') => {
                let s = self.read_string('\'')?;
                Ok(Token::StringLit(s))
            }
            Some('(') => {
                self.advance();
                Ok(Token::LParen)
            }
            Some(')') => {
                self.advance();
                Ok(Token::RParen)
            }
            Some(',') => {
                self.advance();
                Ok(Token::Comma)
            }
            Some('+') => {
                self.advance();
                Ok(Token::Plus)
            }
            Some('-') => {
                self.advance();
                Ok(Token::Minus)
            }
            Some('*') => {
                self.advance();
                Ok(Token::Star)
            }
            Some('/') => {
                self.advance();
                Ok(Token::Slash)
            }
            Some('%') => {
                self.advance();
                Ok(Token::Percent)
            }
            Some('=') => {
                self.advance();
                Ok(Token::Eq)
            }
            Some(ch) if ch.is_ascii_digit() => Ok(self.read_number()),
            Some(ch) if ch.is_ascii_alphabetic() || ch == '_' => Ok(self.read_ident()),
            Some(ch) => Err(PsError {
                message: format!("unexpected character '{}'", ch),
                line: self.line,
                col: self.col,
            }),
        }
    }
}
