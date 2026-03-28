use crate::token::{Token, StringSegment, Tokenizer};
use crate::value::Value;
use crate::scope::Scope;
use crate::PsError;

use indexmap::IndexMap;

/// Control flow signal from block execution.
enum ControlFlow {
    Normal,
    Break,
    Continue,
    Return(Option<Value>),
}

/// A stored user-defined function.
struct UserFunction {
    params: Vec<(String, String)>, // (name, type_name)
    _return_type: Option<String>,
    body: Vec<Token>,
}

/// One-pass executor.
pub struct Executor<'a> {
    stdout: &'a mut dyn std::io::Write,
    stderr: &'a mut dyn std::io::Write,
    peeked: Option<Token>,
    tokenizer: Option<Tokenizer>,
    /// Stack of buffered token streams for block replay.
    token_stack: Vec<(Vec<Token>, usize)>,
    scope: Scope,
    functions: std::collections::HashMap<String, UserFunction>,
    /// Set by exit() builtin.
    exit_code: Option<i32>,
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
            token_stack: Vec::new(),
            scope: Scope::new(),
            functions: std::collections::HashMap::new(),
            exit_code: None,
        }
    }

    // --- Token access ---

    fn next_token(&mut self) -> Result<Token, PsError> {
        if let Some(tok) = self.peeked.take() {
            return Ok(tok);
        }
        // Read from token stack if replaying, otherwise from tokenizer.
        if let Some((tokens, pos)) = self.token_stack.last_mut() {
            if *pos < tokens.len() {
                let tok = tokens[*pos].clone();
                *pos += 1;
                return Ok(tok);
            }
            return Ok(Token::Eof);
        }
        self.tokenizer.as_mut().unwrap().next_token()
    }

    fn peek_token(&mut self) -> Result<&Token, PsError> {
        if self.peeked.is_none() {
            self.peeked = Some(self.next_token()?);
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

    fn next_cmd_token(&mut self) -> Result<Token, PsError> {
        if let Some(tok) = self.peeked.take() {
            return Ok(tok);
        }
        if let Some((tokens, pos)) = self.token_stack.last_mut() {
            if *pos < tokens.len() {
                let tok = tokens[*pos].clone();
                *pos += 1;
                return Ok(tok);
            }
            return Ok(Token::Eof);
        }
        self.tokenizer.as_mut().unwrap().next_cmd_token()
    }

    fn pos(&self) -> (usize, usize) {
        if let Some(ref t) = self.tokenizer {
            (t.line, t.col)
        } else {
            (0, 0)
        }
    }

    // --- Block buffering ---

    /// Buffer tokens until the matching `}` for a `{` block.
    /// Assumes the opening `{` has already been consumed.
    fn buffer_block(&mut self) -> Result<Vec<Token>, PsError> {
        let mut tokens = Vec::new();
        let mut depth = 1;
        loop {
            let tok = self.next_token()?;
            match tok {
                Token::LBrace => {
                    depth += 1;
                    tokens.push(tok);
                }
                Token::RBrace => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(tokens);
                    }
                    tokens.push(tok);
                }
                Token::Eof => {
                    let (line, col) = self.pos();
                    return Err(PsError {
                        message: "unexpected end of file in block".into(),
                        line,
                        col,
                    });
                }
                _ => tokens.push(tok),
            }
        }
    }

    /// Execute a buffered block of tokens.
    fn exec_block(&mut self, tokens: &[Token]) -> Result<ControlFlow, PsError> {
        self.token_stack.push((tokens.to_vec(), 0));
        self.scope.push();
        let result = self.exec_statements();
        self.scope.pop();
        self.token_stack.pop();
        self.peeked = None; // Clear stale peeked tokens from the buffer.
        result
    }

    // --- Main execution loop ---

    /// Execute a source string.
    pub fn run(&mut self, source: &str, args: Vec<String>) -> Result<i32, PsError> {
        // Populate args as a builtin variable.
        let args_list: Vec<Value> = args.into_iter().map(Value::Str).collect();
        self.scope.declare("args", Value::List(args_list), false);

        self.tokenizer = Some(Tokenizer::new(source));
        self.exec_statements()?;
        Ok(self.exit_code.unwrap_or(0))
    }

    /// Execute statements until Eof or a control flow signal.
    fn exec_statements(&mut self) -> Result<ControlFlow, PsError> {
        loop {
            if self.exit_code.is_some() {
                return Ok(ControlFlow::Normal);
            }
            let tok = self.next_token()?;
            if tok == Token::Eof {
                return Ok(ControlFlow::Normal);
            }
            let cf = self.exec_stmt(tok)?;
            match cf {
                ControlFlow::Normal => continue,
                other => return Ok(other),
            }
        }
    }

    /// Execute a single statement given its first token.
    fn exec_stmt(&mut self, tok: Token) -> Result<ControlFlow, PsError> {
        match tok {
            Token::Eof => Ok(ControlFlow::Normal),
            Token::Newline | Token::Comment(_) => Ok(ControlFlow::Normal),
            Token::Let => { self.exec_let()?; Ok(ControlFlow::Normal) }
            Token::Run => { self.exec_run()?; Ok(ControlFlow::Normal) }
            Token::Exec => { self.exec_exec()?; Ok(ControlFlow::Normal) }
            Token::If => self.exec_if(),
            Token::While => self.exec_while(),
            Token::For => self.exec_for(),
            Token::Match => self.exec_match(),
            Token::Fn => { self.exec_fn_def()?; Ok(ControlFlow::Normal) }
            Token::Return => self.exec_return(),
            Token::Break => Ok(ControlFlow::Break),
            Token::Continue => Ok(ControlFlow::Continue),
            Token::Ident(name) => { self.exec_ident_stmt(name)?; Ok(ControlFlow::Normal) }
            _ => {
                let (line, col) = self.pos();
                Err(PsError {
                    message: format!("unexpected token {:?}", tok),
                    line,
                    col,
                })
            }
        }
    }

    // --- Statements ---

    fn exec_let(&mut self) -> Result<(), PsError> {
        let mut mutable = false;
        let tok = self.next_token()?;
        let name = match tok {
            Token::Mut => {
                mutable = true;
                match self.next_token()? {
                    Token::Ident(n) => n,
                    other => {
                        let (line, col) = self.pos();
                        return Err(PsError {
                            message: format!("expected identifier after 'mut', got {:?}", other),
                            line, col,
                        });
                    }
                }
            }
            Token::Ident(n) => n,
            other => {
                let (line, col) = self.pos();
                return Err(PsError {
                    message: format!("expected identifier after 'let', got {:?}", other),
                    line, col,
                });
            }
        };
        self.expect(&Token::Eq)?;
        let value = self.parse_expr()?;
        self.scope.declare(&name, value, mutable);
        Ok(())
    }

    fn exec_run(&mut self) -> Result<(), PsError> {
        let (code, output) = self.exec_run_inner(true)?;
        let stdout_data = output.unwrap_or_default();

        // Check for pipeline.
        if *self.peek_token()? == Token::Pipe {
            return self.exec_pipeline(stdout_data);
        }

        // Write captured output to real stdout.
        write!(self.stdout, "{}", stdout_data).ok();

        let suppress = self.check_question_mark()?;
        if code != 0 && !suppress {
            let (line, col) = self.pos();
            return Err(PsError {
                message: format!("run: exited with code {}", code),
                line, col,
            });
        }
        Ok(())
    }

    fn exec_run_inner(&mut self, capture: bool) -> Result<(i32, Option<String>), PsError> {
        let name_tok = self.next_cmd_token()?;
        let name = match name_tok {
            Token::BareWord(s) | Token::Ident(s) => s,
            other => {
                let (line, col) = self.pos();
                return Err(PsError {
                    message: format!("expected builtin name after 'run', got {:?}", other),
                    line, col,
                });
            }
        };
        let args = self.parse_cmd_args()?;
        let (line, col) = self.pos();
        if capture {
            match crate::builtins::run_builtin_capture(&name, args) {
                Some((code, stdout)) => Ok((code, Some(stdout))),
                None => Err(PsError { message: format!("unknown builtin '{}'", name), line, col }),
            }
        } else {
            match crate::builtins::run_builtin(&name, args) {
                Some(code) => Ok((code, None)),
                None => Err(PsError { message: format!("unknown builtin '{}'", name), line, col }),
            }
        }
    }

    fn exec_exec(&mut self) -> Result<(), PsError> {
        let (code, output) = self.exec_exec_inner(true)?;
        let stdout_data = output.unwrap_or_default();

        // Check for pipeline.
        if *self.peek_token()? == Token::Pipe {
            return self.exec_pipeline(stdout_data);
        }

        // Write captured output to real stdout.
        write!(self.stdout, "{}", stdout_data).ok();

        let suppress = self.check_question_mark()?;
        if code != 0 && !suppress {
            let (line, col) = self.pos();
            return Err(PsError {
                message: format!("exec: exited with code {}", code),
                line, col,
            });
        }
        Ok(())
    }

    fn exec_exec_inner(&mut self, capture: bool) -> Result<(i32, Option<String>), PsError> {
        let args = self.parse_cmd_args()?;
        let (line, col) = self.pos();
        if args.is_empty() {
            return Err(PsError { message: "exec: missing command".into(), line, col });
        }
        let (cmd, cmd_args) = args.split_first().unwrap();
        if capture {
            let output = std::process::Command::new(cmd).args(cmd_args).output()
                .map_err(|e| PsError { message: format!("exec {}: {}", cmd, e), line, col })?;
            let code = output.status.code().unwrap_or(1);
            let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
            Ok((code, Some(stdout)))
        } else {
            let status = std::process::Command::new(cmd).args(cmd_args).status()
                .map_err(|e| PsError { message: format!("exec {}: {}", cmd, e), line, col })?;
            Ok((status.code().unwrap_or(1), None))
        }
    }

    fn exec_if(&mut self) -> Result<ControlFlow, PsError> {
        let cond = self.parse_expr()?;
        self.expect(&Token::LBrace)?;
        let body = self.buffer_block()?;

        if cond.is_truthy() {
            let cf = self.exec_block(&body)?;
            // Skip remaining elif/else blocks.
            self.skip_remaining_elif_else()?;
            return Ok(cf);
        }

        // Check for elif/else chains.
        loop {
            self.skip_newlines()?;
            match self.peek_token()? {
                Token::Elif => {
                    self.next_token()?;
                    let cond = self.parse_expr()?;
                    self.expect(&Token::LBrace)?;
                    let body = self.buffer_block()?;
                    if cond.is_truthy() {
                        // Skip remaining elif/else blocks.
                        self.skip_remaining_elif_else()?;
                        return self.exec_block(&body);
                    }
                }
                Token::Else => {
                    self.next_token()?;
                    self.expect(&Token::LBrace)?;
                    let body = self.buffer_block()?;
                    return self.exec_block(&body);
                }
                _ => break,
            }
        }
        Ok(ControlFlow::Normal)
    }

    fn skip_remaining_elif_else(&mut self) -> Result<(), PsError> {
        loop {
            self.skip_newlines()?;
            match self.peek_token()? {
                Token::Elif => {
                    self.next_token()?;
                    // Skip condition expression tokens until {.
                    self.skip_until_lbrace()?;
                    self.expect(&Token::LBrace)?;
                    self.buffer_block()?;
                }
                Token::Else => {
                    self.next_token()?;
                    self.expect(&Token::LBrace)?;
                    self.buffer_block()?;
                    break;
                }
                _ => break,
            }
        }
        Ok(())
    }

    fn skip_until_lbrace(&mut self) -> Result<(), PsError> {
        loop {
            match self.peek_token()? {
                Token::LBrace => return Ok(()),
                Token::Eof => {
                    let (line, col) = self.pos();
                    return Err(PsError { message: "expected '{'".into(), line, col });
                }
                _ => { self.next_token()?; }
            }
        }
    }

    fn skip_newlines(&mut self) -> Result<(), PsError> {
        loop {
            match self.peek_token()? {
                Token::Newline | Token::Comment(_) => { self.next_token()?; }
                _ => return Ok(()),
            }
        }
    }

    fn exec_while(&mut self) -> Result<ControlFlow, PsError> {
        // We need to buffer the condition expression tokens too, so we can replay them.
        // Simpler approach: buffer everything from here to the closing }.
        // Actually, for one-pass, we buffer the condition tokens and the body tokens separately.

        // Collect condition tokens until {.
        let mut cond_tokens = Vec::new();
        loop {
            let tok = self.next_token()?;
            if tok == Token::LBrace {
                break;
            }
            cond_tokens.push(tok);
        }
        let body = self.buffer_block()?;

        loop {
            // Evaluate condition by replaying cond tokens.
            let cond_val = self.eval_token_expr(&cond_tokens)?;
            if !cond_val.is_truthy() {
                break;
            }
            match self.exec_block(&body)? {
                ControlFlow::Normal | ControlFlow::Continue => continue,
                ControlFlow::Break => break,
                cf @ ControlFlow::Return(_) => return Ok(cf),
            }
        }
        Ok(ControlFlow::Normal)
    }

    fn exec_for(&mut self) -> Result<ControlFlow, PsError> {
        let var_name = match self.next_token()? {
            Token::Ident(s) => s,
            other => {
                let (line, col) = self.pos();
                return Err(PsError {
                    message: format!("expected identifier after 'for', got {:?}", other),
                    line, col,
                });
            }
        };
        self.expect(&Token::In)?;
        let iterable = self.parse_expr()?;
        self.expect(&Token::LBrace)?;
        let body = self.buffer_block()?;

        let items = match iterable {
            Value::List(items) => items,
            other => {
                let (line, col) = self.pos();
                return Err(PsError {
                    message: format!("cannot iterate over {}", other.type_name()),
                    line, col,
                });
            }
        };

        for item in items {
            self.scope.push();
            self.scope.declare(&var_name, item, false);
            self.token_stack.push((body.clone(), 0));
            let cf = self.exec_statements();
            self.token_stack.pop();
            self.scope.pop();
            match cf? {
                ControlFlow::Normal => continue,
                ControlFlow::Continue => continue,
                ControlFlow::Break => break,
                cf @ ControlFlow::Return(_) => return Ok(cf),
            }
        }
        Ok(ControlFlow::Normal)
    }

    fn exec_match(&mut self) -> Result<ControlFlow, PsError> {
        let match_val = self.parse_expr()?;
        self.expect(&Token::LBrace)?;

        // Parse arms until }.
        let mut matched = false;
        loop {
            self.skip_newlines()?;
            if *self.peek_token()? == Token::RBrace {
                self.next_token()?;
                break;
            }

            // Parse pattern(s): expr (| expr)* => stmt_or_block
            let patterns = vec![self.parse_match_pattern()?];
            while *self.peek_token()? == Token::BareWord("_".into()) || false {
                break; // No | handling needed for single patterns.
            }
            // Check for | alternation.
            // Actually, we just parsed one pattern. Now check for |.
            // But | is not a token in expression mode. Let me handle it as a bare check.
            // The `|` is used in pipelines. In match context it's an alternation separator.
            // For now, skip | support in match -- just handle single patterns and _.

            self.expect(&Token::Arrow)?;

            // Parse the arm body: either a block or a single statement.
            self.skip_newlines()?;
            let arm_body = if *self.peek_token()? == Token::LBrace {
                self.next_token()?;
                self.buffer_block()?
            } else {
                // Single statement until newline.
                let mut tokens = Vec::new();
                loop {
                    let tok = self.next_token()?;
                    match tok {
                        Token::Newline | Token::Eof => break,
                        _ => tokens.push(tok),
                    }
                }
                tokens
            };

            if !matched {
                let pattern_matches = match &patterns[0] {
                    MatchPattern::Wildcard => true,
                    MatchPattern::Value(v) => *v == match_val,
                };
                if pattern_matches {
                    matched = true;
                    let cf = self.exec_block(&arm_body)?;
                    match cf {
                        ControlFlow::Normal => {}
                        other => return Ok(other),
                    }
                }
            }
        }
        Ok(ControlFlow::Normal)
    }

    fn parse_match_pattern(&mut self) -> Result<MatchPattern, PsError> {
        let tok = self.peek_token()?;
        if let Token::Ident(s) = tok {
            if s == "_" {
                self.next_token()?;
                return Ok(MatchPattern::Wildcard);
            }
        }
        let val = self.parse_expr()?;
        Ok(MatchPattern::Value(val))
    }

    fn exec_fn_def(&mut self) -> Result<(), PsError> {
        let name = match self.next_token()? {
            Token::Ident(s) => s,
            other => {
                let (line, col) = self.pos();
                return Err(PsError {
                    message: format!("expected function name, got {:?}", other),
                    line, col,
                });
            }
        };

        self.expect(&Token::LParen)?;
        let mut params = Vec::new();
        loop {
            if *self.peek_token()? == Token::RParen {
                self.next_token()?;
                break;
            }
            if !params.is_empty() {
                self.expect(&Token::Comma)?;
            }
            let param_name = match self.next_token()? {
                Token::Ident(s) => s,
                other => {
                    let (line, col) = self.pos();
                    return Err(PsError {
                        message: format!("expected parameter name, got {:?}", other),
                        line, col,
                    });
                }
            };
            self.expect(&Token::Colon)?;
            let type_name = match self.next_token()? {
                Token::Ident(s) => s,
                other => {
                    let (line, col) = self.pos();
                    return Err(PsError {
                        message: format!("expected type name, got {:?}", other),
                        line, col,
                    });
                }
            };
            params.push((param_name, type_name));
        }

        // Optional return type.
        let mut return_type = None;
        if *self.peek_token()? == Token::Minus {
            self.next_token()?; // -
            self.expect(&Token::Gt)?; // >
            return_type = Some(match self.next_token()? {
                Token::Ident(s) => s,
                other => {
                    let (line, col) = self.pos();
                    return Err(PsError {
                        message: format!("expected return type, got {:?}", other),
                        line, col,
                    });
                }
            });
        }

        self.expect(&Token::LBrace)?;
        let body = self.buffer_block()?;

        self.functions.insert(name, UserFunction {
            params,
            _return_type: return_type,
            body,
        });
        Ok(())
    }

    fn exec_return(&mut self) -> Result<ControlFlow, PsError> {
        self.skip_newlines()?;
        // Check if there's a value to return.
        match self.peek_token()? {
            Token::Newline | Token::Eof | Token::RBrace => {
                Ok(ControlFlow::Return(None))
            }
            _ => {
                let val = self.parse_expr()?;
                Ok(ControlFlow::Return(Some(val)))
            }
        }
    }

    fn exec_ident_stmt(&mut self, name: String) -> Result<(), PsError> {
        match self.peek_token()? {
            Token::LParen => {
                self.call_function(&name)?;
                Ok(())
            }
            Token::Eq => {
                self.next_token()?;
                let (line, col) = self.pos();
                let value = self.parse_expr()?;
                self.scope.set(&name, value, line, col)?;
                Ok(())
            }
            Token::LBracket => {
                // Indexed assignment: name[key] = val
                self.next_token()?; // consume [
                let index = self.parse_expr()?;
                self.expect(&Token::RBracket)?;
                self.expect(&Token::Eq)?;
                let value = self.parse_expr()?;
                let (line, col) = self.pos();
                self.scope.index_set(&name, index, value, line, col)?;
                Ok(())
            }
            _ => {
                let (line, col) = self.pos();
                Err(PsError {
                    message: format!("unexpected identifier '{}'", name),
                    line, col,
                })
            }
        }
    }

    // --- Function calls ---

    fn call_function(&mut self, name: &str) -> Result<Option<Value>, PsError> {
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

        // Check builtin functions first.
        if let Some(result) = self.call_builtin_function(name, &args)? {
            return Ok(result);
        }

        // Check user functions.
        let func = self.functions.get(name).ok_or_else(|| {
            let (line, col) = self.pos();
            PsError { message: format!("unknown function '{}'", name), line, col }
        })?;

        let params = func.params.clone();
        let body = func.body.clone();

        if args.len() != params.len() {
            let (line, col) = self.pos();
            return Err(PsError {
                message: format!("{}() takes {} arguments, got {}", name, params.len(), args.len()),
                line, col,
            });
        }

        self.scope.push();
        for ((param_name, _type), val) in params.iter().zip(args) {
            self.scope.declare(param_name, val, false);
        }
        self.token_stack.push((body, 0));
        let cf = self.exec_statements();
        self.token_stack.pop();
        self.scope.pop();

        match cf? {
            ControlFlow::Return(val) => Ok(val),
            _ => Ok(None),
        }
    }

    fn call_builtin_function(&mut self, name: &str, args: &[Value]) -> Result<Option<Option<Value>>, PsError> {
        // Returns Ok(Some(...)) if it's a known builtin, Ok(None) if unknown.
        match name {
            "print" => {
                if args.len() != 1 {
                    let (line, col) = self.pos();
                    return Err(PsError {
                        message: format!("print() takes 1 argument, got {}", args.len()),
                        line, col,
                    });
                }
                write!(self.stdout, "{}", args[0].to_str()).ok();
                Ok(Some(None))
            }
            "eprintln" => {
                if args.len() != 1 {
                    let (line, col) = self.pos();
                    return Err(PsError {
                        message: format!("eprintln() takes 1 argument, got {}", args.len()),
                        line, col,
                    });
                }
                writeln!(self.stderr, "{}", args[0].to_str()).ok();
                Ok(Some(None))
            }

            // Type conversion.
            "int" => {
                let val = &args[0];
                let n = match val {
                    Value::Int(n) => *n,
                    Value::Float(f) => *f as i64,
                    Value::Str(s) => s.parse::<i64>().map_err(|_| {
                        let (line, col) = self.pos();
                        PsError { message: format!("cannot convert '{}' to int", s), line, col }
                    })?,
                    Value::Bool(b) => if *b { 1 } else { 0 },
                    _ => {
                        let (line, col) = self.pos();
                        return Err(PsError { message: format!("cannot convert {} to int", val.type_name()), line, col });
                    }
                };
                Ok(Some(Some(Value::Int(n))))
            }
            "float" => {
                let val = &args[0];
                let f = match val {
                    Value::Float(f) => *f,
                    Value::Int(n) => *n as f64,
                    Value::Str(s) => s.parse::<f64>().map_err(|_| {
                        let (line, col) = self.pos();
                        PsError { message: format!("cannot convert '{}' to float", s), line, col }
                    })?,
                    _ => {
                        let (line, col) = self.pos();
                        return Err(PsError { message: format!("cannot convert {} to float", val.type_name()), line, col });
                    }
                };
                Ok(Some(Some(Value::Float(f))))
            }
            "str" => Ok(Some(Some(Value::Str(args[0].to_str())))),
            "typeof" => Ok(Some(Some(Value::Str(args[0].type_name().to_string())))),

            // String functions.
            "len" => {
                match &args[0] {
                    Value::Str(s) => Ok(Some(Some(Value::Int(s.len() as i64)))),
                    Value::List(l) => Ok(Some(Some(Value::Int(l.len() as i64)))),
                    Value::Map(m) => Ok(Some(Some(Value::Int(m.len() as i64)))),
                    _ => {
                        let (line, col) = self.pos();
                        Err(PsError { message: format!("len() not supported for {}", args[0].type_name()), line, col })
                    }
                }
            }
            "trim" => Ok(Some(Some(Value::Str(args[0].to_str().trim().to_string())))),
            "upper" => Ok(Some(Some(Value::Str(args[0].to_str().to_uppercase())))),
            "lower" => Ok(Some(Some(Value::Str(args[0].to_str().to_lowercase())))),
            "split" => {
                let s = args[0].to_str();
                let delim = args[1].to_str();
                let parts: Vec<Value> = s.split(&delim).map(|p| Value::Str(p.to_string())).collect();
                Ok(Some(Some(Value::List(parts))))
            }
            "join" => {
                match &args[0] {
                    Value::List(items) => {
                        let delim = args[1].to_str();
                        let s: Vec<String> = items.iter().map(|v| v.to_str()).collect();
                        Ok(Some(Some(Value::Str(s.join(&delim)))))
                    }
                    _ => {
                        let (line, col) = self.pos();
                        Err(PsError { message: "join() requires a list".into(), line, col })
                    }
                }
            }
            "lines" => {
                let s = args[0].to_str();
                let parts: Vec<Value> = s.lines().map(|l| Value::Str(l.to_string())).collect();
                Ok(Some(Some(Value::List(parts))))
            }
            "contains" => {
                let s = args[0].to_str();
                let sub = args[1].to_str();
                Ok(Some(Some(Value::Bool(s.contains(&sub)))))
            }
            "starts_with" => {
                Ok(Some(Some(Value::Bool(args[0].to_str().starts_with(&args[1].to_str())))))
            }
            "ends_with" => {
                Ok(Some(Some(Value::Bool(args[0].to_str().ends_with(&args[1].to_str())))))
            }
            "replace" => {
                let s = args[0].to_str();
                let old = args[1].to_str();
                let new = args[2].to_str();
                Ok(Some(Some(Value::Str(s.replace(&old, &new)))))
            }

            // List functions.
            "append" => {
                match &args[0] {
                    Value::List(items) => {
                        let mut new_items = items.clone();
                        new_items.push(args[1].clone());
                        Ok(Some(Some(Value::List(new_items))))
                    }
                    _ => {
                        let (line, col) = self.pos();
                        Err(PsError { message: "append() requires a list".into(), line, col })
                    }
                }
            }
            "range" => {
                let (start, end) = if args.len() == 1 {
                    (0i64, match &args[0] { Value::Int(n) => *n, _ => {
                        let (line, col) = self.pos();
                        return Err(PsError { message: "range() requires int arguments".into(), line, col });
                    }})
                } else {
                    (match &args[0] { Value::Int(n) => *n, _ => {
                        let (line, col) = self.pos();
                        return Err(PsError { message: "range() requires int arguments".into(), line, col });
                    }}, match &args[1] { Value::Int(n) => *n, _ => {
                        let (line, col) = self.pos();
                        return Err(PsError { message: "range() requires int arguments".into(), line, col });
                    }})
                };
                let items: Vec<Value> = (start..end).map(Value::Int).collect();
                Ok(Some(Some(Value::List(items))))
            }

            // Map functions.
            "keys" => {
                match &args[0] {
                    Value::Map(m) => {
                        let ks: Vec<Value> = m.keys().map(|k| Value::Str(k.clone())).collect();
                        Ok(Some(Some(Value::List(ks))))
                    }
                    _ => {
                        let (line, col) = self.pos();
                        Err(PsError { message: "keys() requires a map".into(), line, col })
                    }
                }
            }
            "has_key" => {
                match &args[0] {
                    Value::Map(m) => {
                        let key = args[1].to_str();
                        Ok(Some(Some(Value::Bool(m.contains_key(&key)))))
                    }
                    _ => {
                        let (line, col) = self.pos();
                        Err(PsError { message: "has_key() requires a map".into(), line, col })
                    }
                }
            }

            // Control flow.
            "exit" => {
                let code = if args.is_empty() { 0 } else {
                    match &args[0] {
                        Value::Int(n) => *n as i32,
                        _ => 1,
                    }
                };
                self.exit_code = Some(code);
                Ok(Some(None))
            }
            "error" => {
                let msg = if args.is_empty() { "error".to_string() } else { args[0].to_str() };
                let (line, col) = self.pos();
                Err(PsError { message: msg, line, col })
            }

            _ => Ok(None), // Not a builtin.
        }
    }

    // --- Command args ---

    /// Execute a pipeline. `stdin_data` is the output of the first stage.
    /// Subsequent stages are `| run/exec cmd...` separated by `|`.
    fn exec_pipeline(&mut self, stdin_data: String) -> Result<(), PsError> {
        let mut current_data = stdin_data;

        loop {
            // Consume the | token.
            self.expect(&Token::Pipe)?;

            let tok = self.next_token()?;
            let (code, output) = match tok {
                Token::Run => self.exec_run_pipeline(Some(&current_data))?,
                Token::Exec => self.exec_exec_pipeline(Some(&current_data))?,
                other => {
                    let (line, col) = self.pos();
                    return Err(PsError {
                        message: format!("expected 'run' or 'exec' after '|', got {:?}", other),
                        line, col,
                    });
                }
            };

            current_data = output;

            // Check for more pipeline stages.
            if *self.peek_token()? != Token::Pipe {
                // End of pipeline. Write final output.
                write!(self.stdout, "{}", current_data).ok();
                let suppress = self.check_question_mark()?;
                if code != 0 && !suppress {
                    let (line, col) = self.pos();
                    return Err(PsError {
                        message: format!("pipeline stage failed with code {}", code),
                        line, col,
                    });
                }
                return Ok(());
            }
        }
    }

    /// Run a builtin as a pipeline stage with optional stdin data.
    fn exec_run_pipeline(&mut self, _stdin_data: Option<&str>) -> Result<(i32, String), PsError> {
        let name_tok = self.next_cmd_token()?;
        let name = match name_tok {
            Token::BareWord(s) | Token::Ident(s) => s,
            other => {
                let (line, col) = self.pos();
                return Err(PsError {
                    message: format!("expected builtin name after 'run', got {:?}", other),
                    line, col,
                });
            }
        };
        let args = self.parse_cmd_args()?;
        let (line, col) = self.pos();

        // For pipeline builtins, we use capture mode and feed stdin.
        // TODO: proper stdin feeding for builtins. For now just capture output.
        match crate::builtins::run_builtin_capture(&name, args) {
            Some((code, stdout)) => Ok((code, stdout)),
            None => Err(PsError { message: format!("unknown builtin '{}'", name), line, col }),
        }
    }

    /// Run an external command as a pipeline stage with optional stdin data.
    fn exec_exec_pipeline(&mut self, stdin_data: Option<&str>) -> Result<(i32, String), PsError> {
        let args = self.parse_cmd_args()?;
        let (line, col) = self.pos();
        if args.is_empty() {
            return Err(PsError { message: "exec: missing command".into(), line, col });
        }
        let (cmd, cmd_args) = args.split_first().unwrap();

        let mut child = std::process::Command::new(cmd)
            .args(cmd_args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| PsError { message: format!("exec {}: {}", cmd, e), line, col })?;

        // Feed stdin data.
        if let Some(data) = stdin_data {
            use std::io::Write;
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(data.as_bytes()).ok();
            }
        }

        let output = child.wait_with_output()
            .map_err(|e| PsError { message: format!("exec {}: {}", cmd, e), line, col })?;
        let code = output.status.code().unwrap_or(1);
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        Ok((code, stdout))
    }

    /// Check if the next token is `?` (error suppression). Consumes it if present.
    fn check_question_mark(&mut self) -> Result<bool, PsError> {
        if *self.peek_token()? == Token::Question {
            self.next_token()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn parse_cmd_args(&mut self) -> Result<Vec<String>, PsError> {
        let mut args = Vec::new();
        loop {
            let tok = self.next_cmd_token()?;
            match tok {
                Token::Newline | Token::Eof | Token::Comment(_) => break,
                Token::RParen | Token::Question | Token::Pipe => {
                    self.peeked = Some(tok);
                    break;
                }
                Token::BareWord(s) => args.push(s),
                Token::StringLit(s) => args.push(s),
                Token::StringInterp(segments) => {
                    let val = self.eval_interp_string(segments)?;
                    args.push(val.to_str());
                }
                Token::LBrace => {
                    let val = self.parse_expr()?;
                    self.expect(&Token::RBrace)?;
                    args.push(val.to_str());
                }
                other => {
                    let (line, col) = self.pos();
                    return Err(PsError {
                        message: format!("unexpected token in command args: {:?}", other),
                        line, col,
                    });
                }
            }
        }
        Ok(args)
    }

    // --- Expressions ---

    pub fn parse_expr(&mut self) -> Result<Value, PsError> {
        self.parse_expr_bp(0)
    }

    fn parse_expr_bp(&mut self, min_bp: u8) -> Result<Value, PsError> {
        let mut lhs = self.parse_unary()?;

        loop {
            let (op, is_cmp, is_logic) = match self.peek_token()? {
                Token::Plus => ("+", false, false),
                Token::Minus => ("-", false, false),
                Token::Star => ("*", false, false),
                Token::Slash => ("/", false, false),
                Token::Percent => ("%", false, false),
                Token::EqEq => ("==", true, false),
                Token::NotEq => ("!=", true, false),
                Token::Lt => ("<", true, false),
                Token::Gt => (">", true, false),
                Token::LtEq => ("<=", true, false),
                Token::GtEq => (">=", true, false),
                Token::And => ("and", false, true),
                Token::Or => ("or", false, true),
                Token::QuestionQuestion => ("??", false, true),
                _ => break,
            };

            let (l_bp, r_bp) = infix_binding_power(op);
            if l_bp < min_bp {
                break;
            }
            self.next_token()?;

            if is_logic {
                let rhs = self.parse_expr_bp(r_bp)?;
                lhs = match op {
                    "and" => Value::Bool(lhs.is_truthy() && rhs.is_truthy()),
                    "or" => Value::Bool(lhs.is_truthy() || rhs.is_truthy()),
                    "??" => {
                        // Coalesce: return lhs if non-empty string, else rhs.
                        match &lhs {
                            Value::Str(s) if !s.is_empty() => lhs,
                            _ => rhs,
                        }
                    }
                    _ => unreachable!(),
                };
            } else if is_cmp {
                let rhs = self.parse_expr_bp(r_bp)?;
                let (line, col) = self.pos();
                lhs = lhs.compare(op, &rhs).map_err(|msg| PsError { message: msg, line, col })?;
            } else {
                let rhs = self.parse_expr_bp(r_bp)?;
                let (line, col) = self.pos();
                lhs = lhs.binary_op(op, rhs).map_err(|msg| PsError { message: msg, line, col })?;
            }
        }

        Ok(lhs)
    }

    fn parse_unary(&mut self) -> Result<Value, PsError> {
        if *self.peek_token()? == Token::Not {
            self.next_token()?;
            let val = self.parse_unary()?;
            return Ok(Value::Bool(!val.is_truthy()));
        }
        let mut val = self.parse_primary()?;
        // Postfix: dot access.
        while *self.peek_token()? == Token::Dot {
            self.next_token()?; // consume '.'
            let field = match self.next_token()? {
                Token::Ident(s) => s,
                other => {
                    let (line, col) = self.pos();
                    return Err(PsError {
                        message: format!("expected field name after '.', got {:?}", other),
                        line, col,
                    });
                }
            };
            match val {
                Value::Map(ref m) => {
                    val = m.get(&field).cloned().ok_or_else(|| {
                        let (line, col) = self.pos();
                        PsError { message: format!("map has no field '{}'", field), line, col }
                    })?;
                }
                _ => {
                    let (line, col) = self.pos();
                    return Err(PsError {
                        message: format!("cannot access field '{}' on {}", field, val.type_name()),
                        line, col,
                    });
                }
            }
        }
        // Postfix: index access [i].
        while *self.peek_token()? == Token::LBracket {
            self.next_token()?; // consume '['
            let index = self.parse_expr()?;
            self.expect(&Token::RBracket)?;
            let (line, col) = self.pos();
            match (&val, &index) {
                (Value::List(items), Value::Int(i)) => {
                    let idx = if *i < 0 { items.len() as i64 + i } else { *i } as usize;
                    val = items.get(idx).cloned().ok_or_else(|| {
                        PsError { message: format!("index {} out of bounds (len {})", i, items.len()), line, col }
                    })?;
                }
                (Value::Map(m), Value::Str(key)) => {
                    val = m.get(key).cloned().ok_or_else(|| {
                        PsError { message: format!("key '{}' not found in map", key), line, col }
                    })?;
                }
                _ => {
                    return Err(PsError {
                        message: format!("cannot index {} with {}", val.type_name(), index.type_name()),
                        line, col,
                    });
                }
            }
        }
        Ok(val)
    }

    fn parse_primary(&mut self) -> Result<Value, PsError> {
        let tok = self.next_token()?;
        match tok {
            Token::StringLit(s) => Ok(Value::Str(s)),
            Token::StringInterp(segments) => self.eval_interp_string(segments),
            Token::IntLit(n) => Ok(Value::Int(n)),
            Token::FloatLit(f) => Ok(Value::Float(f)),
            Token::True => Ok(Value::Bool(true)),
            Token::False => Ok(Value::Bool(false)),
            Token::LParen => {
                let val = self.parse_expr()?;
                self.expect(&Token::RParen)?;
                Ok(val)
            }
            Token::LBracket => {
                // List literal [a, b, c].
                let mut items = Vec::new();
                loop {
                    if *self.peek_token()? == Token::RBracket {
                        self.next_token()?;
                        break;
                    }
                    if !items.is_empty() {
                        self.expect(&Token::Comma)?;
                    }
                    // Skip newlines inside list literals.
                    self.skip_newlines()?;
                    if *self.peek_token()? == Token::RBracket {
                        self.next_token()?;
                        break;
                    }
                    items.push(self.parse_expr()?);
                }
                Ok(Value::List(items))
            }
            Token::DollarParen => self.parse_capture(),
            Token::Try => self.parse_try(),
            Token::Env => {
                // env.VAR_NAME
                self.expect(&Token::Dot)?;
                let var_name = match self.next_token()? {
                    Token::Ident(s) => s,
                    other => {
                        let (line, col) = self.pos();
                        return Err(PsError {
                            message: format!("expected env var name after 'env.', got {:?}", other),
                            line, col,
                        });
                    }
                };
                let val = std::env::var(&var_name).unwrap_or_default();
                Ok(Value::Str(val))
            }
            Token::LBrace => {
                // Map literal {key: val, ...}.
                let mut map = IndexMap::new();
                loop {
                    self.skip_newlines()?;
                    if *self.peek_token()? == Token::RBrace {
                        self.next_token()?;
                        break;
                    }
                    if !map.is_empty() {
                        self.expect(&Token::Comma)?;
                        self.skip_newlines()?;
                        if *self.peek_token()? == Token::RBrace {
                            self.next_token()?;
                            break;
                        }
                    }
                    let key = match self.next_token()? {
                        Token::Ident(s) => s,
                        Token::StringLit(s) => s,
                        other => {
                            let (line, col) = self.pos();
                            return Err(PsError {
                                message: format!("expected map key, got {:?}", other),
                                line, col,
                            });
                        }
                    };
                    self.expect(&Token::Colon)?;
                    let val = self.parse_expr()?;
                    map.insert(key, val);
                }
                Ok(Value::Map(map))
            }
            Token::Ident(name) => {
                if *self.peek_token()? == Token::LParen {
                    match self.call_function(&name)? {
                        Some(val) => Ok(val),
                        None => {
                            let (line, col) = self.pos();
                            Err(PsError {
                                message: format!("function '{}' does not return a value", name),
                                line, col,
                            })
                        }
                    }
                } else {
                    match self.scope.get(&name) {
                        Some(val) => Ok(val.clone()),
                        None => {
                            let (line, col) = self.pos();
                            Err(PsError {
                                message: format!("undefined variable '{}'", name),
                                line, col,
                            })
                        }
                    }
                }
            }
            _ => {
                let (line, col) = self.pos();
                Err(PsError {
                    message: format!("expected expression, got {:?}", tok),
                    line, col,
                })
            }
        }
    }

    fn parse_capture(&mut self) -> Result<Value, PsError> {
        let tok = self.next_token()?;
        let (mut code, captured) = match tok {
            Token::Run => self.exec_run_inner(true)?,
            Token::Exec => self.exec_exec_inner(true)?,
            other => {
                let (line, col) = self.pos();
                return Err(PsError {
                    message: format!("expected 'run' or 'exec' after '$(', got {:?}", other),
                    line, col,
                });
            }
        };
        let mut current_data = captured.unwrap_or_default();

        // Handle pipeline stages within $().
        while *self.peek_token()? == Token::Pipe {
            self.next_token()?; // consume |
            let tok = self.next_token()?;
            let (stage_code, stage_output) = match tok {
                Token::Run => self.exec_run_pipeline(Some(&current_data))?,
                Token::Exec => self.exec_exec_pipeline(Some(&current_data))?,
                other => {
                    let (line, col) = self.pos();
                    return Err(PsError {
                        message: format!("expected 'run' or 'exec' after '|', got {:?}", other),
                        line, col,
                    });
                }
            };
            code = stage_code;
            current_data = stage_output;
        }

        self.expect(&Token::RParen)?;
        if code != 0 {
            let (line, col) = self.pos();
            return Err(PsError {
                message: format!("command in $() failed with code {}", code),
                line, col,
            });
        }
        Ok(Value::Str(current_data.trim_end().to_string()))
    }

    /// Parse `try run/exec cmd` -- captures result as a map.
    fn parse_try(&mut self) -> Result<Value, PsError> {
        let tok = self.next_token()?;
        let (mut code, captured) = match tok {
            Token::Run => self.exec_run_inner(true)?,
            Token::Exec => self.exec_exec_inner(true)?,
            other => {
                let (line, col) = self.pos();
                return Err(PsError {
                    message: format!("expected 'run' or 'exec' after 'try', got {:?}", other),
                    line, col,
                });
            }
        };
        let mut current_data = captured.unwrap_or_default();

        // Handle pipeline stages within try.
        while *self.peek_token()? == Token::Pipe {
            self.next_token()?;
            let tok = self.next_token()?;
            let (stage_code, stage_output) = match tok {
                Token::Run => self.exec_run_pipeline(Some(&current_data))?,
                Token::Exec => self.exec_exec_pipeline(Some(&current_data))?,
                other => {
                    let (line, col) = self.pos();
                    return Err(PsError {
                        message: format!("expected 'run' or 'exec' after '|', got {:?}", other),
                        line, col,
                    });
                }
            };
            code = stage_code;
            current_data = stage_output;
        }

        let mut map = IndexMap::new();
        map.insert("ok".into(), Value::Bool(code == 0));
        map.insert("code".into(), Value::Int(code as i64));
        map.insert("stdout".into(), Value::Str(current_data));
        map.insert("stderr".into(), Value::Str(String::new())); // TODO: capture stderr
        Ok(Value::Map(map))
    }

    fn eval_interp_string(&mut self, segments: Vec<StringSegment>) -> Result<Value, PsError> {
        let mut result = String::new();
        for seg in segments {
            match seg {
                StringSegment::Literal(s) => result.push_str(&s),
                StringSegment::Expr(expr_src) => {
                    // Tokenize the expression and evaluate via token stack.
                    let mut tok = Tokenizer::new(&expr_src);
                    let mut tokens = Vec::new();
                    loop {
                        let t = tok.next_token()?;
                        if t == Token::Eof {
                            break;
                        }
                        tokens.push(t);
                    }
                    let val = self.eval_token_expr(&tokens)?;
                    result.push_str(&val.to_str());
                }
            }
        }
        Ok(Value::Str(result))
    }

    /// Evaluate an expression from buffered tokens.
    fn eval_token_expr(&mut self, tokens: &[Token]) -> Result<Value, PsError> {
        self.token_stack.push((tokens.to_vec(), 0));
        let val = self.parse_expr();
        self.token_stack.pop();
        self.peeked = None; // Clear any leftover peeked token from the buffer.
        val
    }
}

enum MatchPattern {
    Value(Value),
    Wildcard,
}

fn infix_binding_power(op: &str) -> (u8, u8) {
    match op {
        "??" => (1, 2),
        "or" => (3, 4),
        "and" => (5, 6),
        "==" | "!=" | "<" | ">" | "<=" | ">=" => (7, 8),
        "+" | "-" => (9, 10),
        "*" | "/" | "%" => (11, 12),
        _ => (0, 0),
    }
}
