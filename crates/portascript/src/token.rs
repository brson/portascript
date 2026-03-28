use crate::PsError;

/// A segment of an interpolated string.
#[derive(Debug, Clone, PartialEq)]
pub enum StringSegment {
    /// Literal text.
    Literal(String),
    /// An expression to evaluate (source text between `{` and `}`).
    Expr(String),
}

/// Token types for the portascript lexer.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Eof,
    Newline,
    Comment(String),

    // Literals.
    /// Raw string (single-quoted, no interpolation).
    StringLit(String),
    /// Interpolated string (double-quoted). Segments alternate between
    /// literal text and expression source fragments.
    StringInterp(Vec<StringSegment>),
    IntLit(i64),
    FloatLit(f64),

    // Identifiers and keywords.
    Ident(String),
    True,
    False,
    Run,
    Exec,
    Let,
    Mut,
    If,
    Elif,
    Else,
    While,
    For,
    In,
    Break,
    Continue,
    Fn,
    Return,
    Match,
    Try,
    Env,
    And,
    Or,
    Not,

    // Operators.
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    EqEq,
    NotEq,
    Lt,
    Gt,
    LtEq,
    GtEq,
    Arrow,    // =>
    Colon,
    Dot,
    DotDot,
    Question,
    Pipe,
    QuestionQuestion, // ??

    /// A bare word in command mode (flags, paths, etc.).
    BareWord(String),

    // Symbols.
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    /// `$(` -- start of command capture expression.
    DollarParen,
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

    /// Read a raw string (single-quoted, no interpolation).
    fn read_raw_string(&mut self) -> Result<String, PsError> {
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
                Some('\'') => return Ok(s),
                Some(ch) => s.push(ch),
            }
        }
    }

    /// Read a double-quoted string with `{expr}` interpolation.
    fn read_interp_string(&mut self) -> Result<Token, PsError> {
        let (line, col) = (self.line, self.col);
        self.advance(); // consume opening "
        let mut segments = Vec::new();
        let mut current = String::new();
        let mut has_interp = false;
        loop {
            match self.advance() {
                None => {
                    return Err(PsError {
                        message: "unterminated string".into(),
                        line,
                        col,
                    });
                }
                Some('"') => {
                    if !current.is_empty() {
                        segments.push(StringSegment::Literal(current));
                    }
                    break;
                }
                Some('{') => {
                    if !current.is_empty() {
                        segments.push(StringSegment::Literal(current));
                        current = String::new();
                    }
                    // Read until matching '}'.
                    let mut expr = String::new();
                    let mut depth = 1;
                    loop {
                        match self.advance() {
                            None => {
                                return Err(PsError {
                                    message: "unterminated interpolation in string".into(),
                                    line,
                                    col,
                                });
                            }
                            Some('{') => {
                                depth += 1;
                                expr.push('{');
                            }
                            Some('}') => {
                                depth -= 1;
                                if depth == 0 {
                                    break;
                                }
                                expr.push('}');
                            }
                            Some(ch) => expr.push(ch),
                        }
                    }
                    segments.push(StringSegment::Expr(expr));
                    has_interp = true;
                }
                Some('\\') => {
                    // Escape sequence.
                    match self.advance() {
                        Some('n') => current.push('\n'),
                        Some('t') => current.push('\t'),
                        Some('\\') => current.push('\\'),
                        Some('{') => current.push('{'),
                        Some('"') => current.push('"'),
                        Some(ch) => {
                            current.push('\\');
                            current.push(ch);
                        }
                        None => current.push('\\'),
                    }
                }
                Some(ch) => current.push(ch),
            }
        }
        // Optimization: if no interpolation, produce a plain StringLit.
        if !has_interp {
            let s = segments
                .into_iter()
                .map(|seg| match seg {
                    StringSegment::Literal(s) => s,
                    StringSegment::Expr(_) => unreachable!(),
                })
                .collect::<String>();
            Ok(Token::StringLit(s))
        } else {
            Ok(Token::StringInterp(segments))
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
            "run" => Token::Run,
            "exec" => Token::Exec,
            "let" => Token::Let,
            "mut" => Token::Mut,
            "if" => Token::If,
            "elif" => Token::Elif,
            "else" => Token::Else,
            "while" => Token::While,
            "for" => Token::For,
            "in" => Token::In,
            "break" => Token::Break,
            "continue" => Token::Continue,
            "fn" => Token::Fn,
            "return" => Token::Return,
            "match" => Token::Match,
            "try" => Token::Try,
            "env" => Token::Env,
            "and" => Token::And,
            "or" => Token::Or,
            "not" => Token::Not,
            _ => Token::Ident(s),
        }
    }

    /// Read the next token in command mode.
    ///
    /// In command mode, bare words (flags, paths) are valid tokens.
    /// Stops at newline, `|`, or EOF.
    /// Read a triple-quoted string (""" or ''').
    /// If `interpolate` is true, processes `{expr}` and escape sequences (""").
    /// If false, raw (''').
    fn read_triple_string(&mut self, interpolate: bool) -> Result<Token, PsError> {
        let (line, col) = (self.line, self.col);
        let quote = self.peek().unwrap();
        // Consume the three opening quotes.
        self.advance();
        self.advance();
        self.advance();
        // Skip optional newline after opening quotes.
        if self.peek() == Some('\n') {
            self.advance();
        }

        let mut content = String::new();
        loop {
            match self.peek() {
                None => {
                    return Err(PsError {
                        message: "unterminated triple-quoted string".into(),
                        line, col,
                    });
                }
                Some(ch) if ch == quote => {
                    // Check for closing triple quotes.
                    if self.chars.get(self.pos + 1) == Some(&quote)
                        && self.chars.get(self.pos + 2) == Some(&quote)
                    {
                        self.advance();
                        self.advance();
                        self.advance();
                        break;
                    }
                    content.push(ch);
                    self.advance();
                }
                Some('\\') if interpolate => {
                    self.advance();
                    match self.advance() {
                        Some('n') => content.push('\n'),
                        Some('t') => content.push('\t'),
                        Some('\\') => content.push('\\'),
                        Some('{') => content.push('{'),
                        Some('"') => content.push('"'),
                        Some(ch) => { content.push('\\'); content.push(ch); }
                        None => content.push('\\'),
                    }
                }
                Some(ch) => {
                    content.push(ch);
                    self.advance();
                }
            }
        }

        // Strip leading whitespace based on closing quotes indentation.
        let content = Self::dedent_triple_string(&content);

        // TODO: interpolation in triple-quoted strings. For now, return as plain string.
        Ok(Token::StringLit(content))
    }

    /// Dedent a triple-quoted string based on the indentation of the last line.
    fn dedent_triple_string(s: &str) -> String {
        let lines: Vec<&str> = s.lines().collect();
        if lines.is_empty() {
            return String::new();
        }
        // The last line (before closing """) determines the indent to strip.
        let last = lines.last().unwrap();
        let indent = last.len() - last.trim_start().len();
        // If the last line is all whitespace, it's the indent marker.
        let (content_lines, strip) = if last.trim().is_empty() {
            (&lines[..lines.len() - 1], indent)
        } else {
            (&lines[..], indent)
        };
        content_lines
            .iter()
            .map(|line| {
                if line.len() >= strip && line[..strip].chars().all(|c| c == ' ') {
                    &line[strip..]
                } else {
                    line.trim_start()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn next_cmd_token(&mut self) -> Result<Token, PsError> {
        self.skip_whitespace_no_newline();

        match self.peek() {
            None => Ok(Token::Eof),
            Some('\n') | Some(';') => {
                self.advance();
                Ok(Token::Newline)
            }
            Some('#') => {
                // Comment consumes to end of line.
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
            Some('"') => self.read_interp_string(),
            Some('\'') => {
                let s = self.read_raw_string()?;
                Ok(Token::StringLit(s))
            }
            Some('{') => {
                self.advance();
                Ok(Token::LBrace)
            }
            Some(')') => {
                self.advance();
                Ok(Token::RParen)
            }
            Some('?') => {
                self.advance();
                Ok(Token::Question)
            }
            Some('|') => {
                self.advance();
                Ok(Token::Pipe)
            }
            _ => {
                // Read a bare word: anything that isn't whitespace, newline, or special chars.
                let mut word = String::new();
                while let Some(ch) = self.peek() {
                    if ch == ' ' || ch == '\t' || ch == '\r' || ch == '\n'
                        || ch == '{' || ch == '"' || ch == '\'' || ch == '#'
                        || ch == ')' || ch == '?' || ch == '|'
                    {
                        break;
                    }
                    word.push(ch);
                    self.advance();
                }
                Ok(Token::BareWord(word))
            }
        }
    }

    /// Read the next token.
    pub fn next_token(&mut self) -> Result<Token, PsError> {
        self.skip_whitespace_no_newline();

        // Line continuation: backslash at end of line.
        if self.peek() == Some('\\') {
            if self.chars.get(self.pos + 1) == Some(&'\n') {
                self.advance(); // consume '\'
                self.advance(); // consume '\n'
                self.skip_whitespace_no_newline();
                return self.next_token();
            }
        }

        match self.peek() {
            None => Ok(Token::Eof),
            Some('\n') | Some(';') => {
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
                // Check for triple-quoted string """.
                if self.chars.get(self.pos + 1) == Some(&'"') && self.chars.get(self.pos + 2) == Some(&'"') {
                    self.read_triple_string(true)
                } else {
                    self.read_interp_string()
                }
            }
            Some('\'') => {
                // Check for triple-quoted raw string '''.
                if self.chars.get(self.pos + 1) == Some(&'\'') && self.chars.get(self.pos + 2) == Some(&'\'') {
                    self.read_triple_string(false)
                } else {
                    let s = self.read_raw_string()?;
                    Ok(Token::StringLit(s))
                }
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
            Some('$') => {
                self.advance();
                if self.peek() == Some('(') {
                    self.advance();
                    Ok(Token::DollarParen)
                } else {
                    Err(PsError {
                        message: "unexpected '$' (did you mean '$('?)".into(),
                        line: self.line,
                        col: self.col,
                    })
                }
            }
            Some('{') => {
                self.advance();
                Ok(Token::LBrace)
            }
            Some('}') => {
                self.advance();
                Ok(Token::RBrace)
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
                if self.peek() == Some('=') {
                    self.advance();
                    Ok(Token::EqEq)
                } else if self.peek() == Some('>') {
                    self.advance();
                    Ok(Token::Arrow)
                } else {
                    Ok(Token::Eq)
                }
            }
            Some('!') => {
                self.advance();
                if self.peek() == Some('=') {
                    self.advance();
                    Ok(Token::NotEq)
                } else {
                    Err(PsError {
                        message: "unexpected '!' (did you mean '!='?)".into(),
                        line: self.line,
                        col: self.col,
                    })
                }
            }
            Some('<') => {
                self.advance();
                if self.peek() == Some('=') {
                    self.advance();
                    Ok(Token::LtEq)
                } else {
                    Ok(Token::Lt)
                }
            }
            Some('>') => {
                self.advance();
                if self.peek() == Some('=') {
                    self.advance();
                    Ok(Token::GtEq)
                } else {
                    Ok(Token::Gt)
                }
            }
            Some(':') => {
                self.advance();
                Ok(Token::Colon)
            }
            Some('[') => {
                self.advance();
                Ok(Token::LBracket)
            }
            Some(']') => {
                self.advance();
                Ok(Token::RBracket)
            }
            Some('.') => {
                self.advance();
                if self.peek() == Some('.') {
                    self.advance();
                    Ok(Token::DotDot)
                } else {
                    Ok(Token::Dot)
                }
            }
            Some('?') => {
                self.advance();
                if self.peek() == Some('?') {
                    self.advance();
                    Ok(Token::QuestionQuestion)
                } else {
                    Ok(Token::Question)
                }
            }
            Some('|') => {
                self.advance();
                Ok(Token::Pipe)
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
