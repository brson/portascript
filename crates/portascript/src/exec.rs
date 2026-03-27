use crate::token::{Token, Tokenizer};
use crate::value::Value;
use crate::scope::Scope;
use crate::PsError;

/// One-pass executor.
pub struct Executor<'a> {
    stdout: &'a mut dyn std::io::Write,
    stderr: &'a mut dyn std::io::Write,
    peeked: Option<Token>,
    tokenizer: Option<Tokenizer>,
    scope: Scope,
}

impl<'a> Executor<'a> {
    pub fn new(
        stdout: &'a mut dyn std::io::Write,
        stderr: &'a mut dyn std::io::Write,
    ) -> Self {
        Self {
            stdout,
            stderr,
            peeked: None,
            tokenizer: None,
            scope: Scope::new(),
        }
    }

    fn next_token(&mut self) -> Result<Token, PsError> {
        if let Some(tok) = self.peeked.take() {
            return Ok(tok);
        }
        self.tokenizer.as_mut().unwrap().next_token()
    }

    fn peek_token(&mut self) -> Result<&Token, PsError> {
        if self.peeked.is_none() {
            self.peeked = Some(self.tokenizer.as_mut().unwrap().next_token()?);
        }
        Ok(self.peeked.as_ref().unwrap())
    }

    fn expect(&mut self, expected: &Token) -> Result<(), PsError> {
        let tok = self.next_token()?;
        if &tok != expected {
            let (line, col) = self.pos();
            return Err(PsError {
                message: format!("expected {:?}, got {:?}", expected, tok),
                line,
                col,
            });
        }
        Ok(())
    }

    fn pos(&self) -> (usize, usize) {
        if let Some(ref t) = self.tokenizer {
            (t.line, t.col)
        } else {
            (0, 0)
        }
    }

    /// Execute a source string.
    pub fn run(&mut self, source: &str) -> Result<i32, PsError> {
        self.tokenizer = Some(Tokenizer::new(source));
        loop {
            let tok = self.next_token()?;
            match tok {
                Token::Eof => return Ok(0),
                Token::Newline | Token::Comment(_) => continue,
                Token::Ident(ref name) if name == "let" => {
                    self.exec_let()?;
                }
                Token::Ident(name) => {
                    self.exec_ident_stmt(name)?;
                }
                _ => {
                    let (line, col) = self.pos();
                    return Err(PsError {
                        message: format!("unexpected token {:?}", tok),
                        line,
                        col,
                    });
                }
            }
        }
    }

    /// Execute `let [mut] name = expr`.
    fn exec_let(&mut self) -> Result<(), PsError> {
        let mut mutable = false;
        let tok = self.next_token()?;
        let name = match tok {
            Token::Ident(ref s) if s == "mut" => {
                mutable = true;
                match self.next_token()? {
                    Token::Ident(n) => n,
                    other => {
                        let (line, col) = self.pos();
                        return Err(PsError {
                            message: format!("expected identifier after 'mut', got {:?}", other),
                            line,
                            col,
                        });
                    }
                }
            }
            Token::Ident(n) => n,
            other => {
                let (line, col) = self.pos();
                return Err(PsError {
                    message: format!("expected identifier after 'let', got {:?}", other),
                    line,
                    col,
                });
            }
        };
        self.expect(&Token::Eq)?;
        let value = self.parse_expr()?;
        self.scope.declare(&name, value, mutable);
        Ok(())
    }

    /// Handle a statement starting with an identifier: function call or assignment.
    fn exec_ident_stmt(&mut self, name: String) -> Result<(), PsError> {
        match self.peek_token()? {
            Token::LParen => {
                self.call_builtin_function(&name)?;
                Ok(())
            }
            Token::Eq => {
                // Assignment.
                self.next_token()?; // consume '='
                let (line, col) = self.pos();
                let value = self.parse_expr()?;
                self.scope.set(&name, value, line, col)?;
                Ok(())
            }
            _ => {
                let (line, col) = self.pos();
                Err(PsError {
                    message: format!("unexpected identifier '{}'", name),
                    line,
                    col,
                })
            }
        }
    }

    fn call_builtin_function(&mut self, name: &str) -> Result<Option<Value>, PsError> {
        self.expect(&Token::LParen)?;
        let mut args = Vec::new();
        loop {
            if *self.peek_token()? == Token::RParen {
                self.next_token()?;
                break;
            }
            if !args.is_empty() {
                self.expect(&Token::Comma)?;
            }
            args.push(self.parse_expr()?);
        }

        match name {
            "print" => {
                if args.len() != 1 {
                    let (line, col) = self.pos();
                    return Err(PsError {
                        message: format!("print() takes 1 argument, got {}", args.len()),
                        line,
                        col,
                    });
                }
                write!(self.stdout, "{}", args[0].to_str()).ok();
                Ok(None)
            }
            _ => {
                let (line, col) = self.pos();
                Err(PsError {
                    message: format!("unknown function '{}'", name),
                    line,
                    col,
                })
            }
        }
    }

    /// Parse an expression with operator precedence (precedence climbing).
    fn parse_expr(&mut self) -> Result<Value, PsError> {
        self.parse_expr_bp(0)
    }

    /// Precedence climbing: parse expression with minimum binding power.
    fn parse_expr_bp(&mut self, min_bp: u8) -> Result<Value, PsError> {
        let mut lhs = self.parse_primary()?;

        loop {
            let op = match self.peek_token()? {
                Token::Plus => "+",
                Token::Minus => "-",
                Token::Star => "*",
                Token::Slash => "/",
                Token::Percent => "%",
                _ => break,
            };

            let (l_bp, r_bp) = infix_binding_power(op);
            if l_bp < min_bp {
                break;
            }
            self.next_token()?; // consume operator

            let rhs = self.parse_expr_bp(r_bp)?;
            let (line, col) = self.pos();
            lhs = lhs.binary_op(op, rhs).map_err(|msg| PsError { message: msg, line, col })?;
        }

        Ok(lhs)
    }

    /// Parse a primary expression (literal, variable, function call, parenthesized).
    fn parse_primary(&mut self) -> Result<Value, PsError> {
        let tok = self.next_token()?;
        match tok {
            Token::StringLit(s) => Ok(Value::Str(s)),
            Token::IntLit(n) => Ok(Value::Int(n)),
            Token::FloatLit(f) => Ok(Value::Float(f)),
            Token::True => Ok(Value::Bool(true)),
            Token::False => Ok(Value::Bool(false)),
            Token::LParen => {
                let val = self.parse_expr()?;
                self.expect(&Token::RParen)?;
                Ok(val)
            }
            Token::Ident(name) => {
                // Check for function call.
                if *self.peek_token()? == Token::LParen {
                    match self.call_builtin_function(&name)? {
                        Some(val) => Ok(val),
                        None => {
                            let (line, col) = self.pos();
                            Err(PsError {
                                message: format!("function '{}' does not return a value", name),
                                line,
                                col,
                            })
                        }
                    }
                } else {
                    // Variable reference.
                    match self.scope.get(&name) {
                        Some(val) => Ok(val.clone()),
                        None => {
                            let (line, col) = self.pos();
                            Err(PsError {
                                message: format!("undefined variable '{}'", name),
                                line,
                                col,
                            })
                        }
                    }
                }
            }
            _ => {
                let (line, col) = self.pos();
                Err(PsError {
                    message: format!("expected expression, got {:?}", tok),
                    line,
                    col,
                })
            }
        }
    }
}

/// Return (left binding power, right binding power) for infix operators.
fn infix_binding_power(op: &str) -> (u8, u8) {
    match op {
        "+" | "-" => (1, 2),
        "*" | "/" | "%" => (3, 4),
        _ => (0, 0),
    }
}
