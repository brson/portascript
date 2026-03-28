use indexmap::IndexMap;

/// Runtime value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Str(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    List(Vec<Value>),
    Map(IndexMap<String, Value>),
}

impl Value {
    /// Coerce to string representation.
    pub fn to_str(&self) -> String {
        match self {
            Value::Str(s) => s.clone(),
            Value::Int(n) => n.to_string(),
            Value::Float(f) => f.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::List(items) => {
                let parts: Vec<String> = items.iter().map(|v| v.to_str()).collect();
                format!("[{}]", parts.join(", "))
            }
            Value::Map(m) => {
                let parts: Vec<String> = m.iter().map(|(k, v)| format!("{}: {}", k, v.to_str())).collect();
                format!("{{{}}}", parts.join(", "))
            }
        }
    }

    /// Apply a binary arithmetic/string operator.
    pub fn binary_op(self, op: &str, other: Value) -> Result<Value, String> {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => match op {
                "+" => Ok(Value::Int(a + b)),
                "-" => Ok(Value::Int(a - b)),
                "*" => Ok(Value::Int(a * b)),
                "/" => {
                    if b == 0 { Err("division by zero".into()) }
                    else { Ok(Value::Int(a / b)) }
                }
                "%" => {
                    if b == 0 { Err("division by zero".into()) }
                    else { Ok(Value::Int(a % b)) }
                }
                _ => Err(format!("unsupported operator '{}' for int", op)),
            },
            (Value::Float(a), Value::Float(b)) => match op {
                "+" => Ok(Value::Float(a + b)),
                "-" => Ok(Value::Float(a - b)),
                "*" => Ok(Value::Float(a * b)),
                "/" => Ok(Value::Float(a / b)),
                "%" => Ok(Value::Float(a % b)),
                _ => Err(format!("unsupported operator '{}' for float", op)),
            },
            (Value::Int(a), Value::Float(b)) => Value::Float(a as f64).binary_op(op, Value::Float(b)),
            (Value::Float(a), Value::Int(b)) => Value::Float(a).binary_op(op, Value::Float(b as f64)),
            (Value::Str(a), Value::Str(b)) if op == "+" => Ok(Value::Str(a + &b)),
            (a, b) => Err(format!("unsupported operator '{}' between {} and {}", op, a.type_name(), b.type_name())),
        }
    }

    /// Apply a comparison operator. Returns a Bool.
    pub fn compare(&self, op: &str, other: &Value) -> Result<Value, String> {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Bool(int_cmp(*a, *b, op))),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Bool(float_cmp(*a, *b, op))),
            (Value::Int(a), Value::Float(b)) => Ok(Value::Bool(float_cmp(*a as f64, *b, op))),
            (Value::Float(a), Value::Int(b)) => Ok(Value::Bool(float_cmp(*a, *b as f64, op))),
            (Value::Str(a), Value::Str(b)) => Ok(Value::Bool(str_cmp(a, b, op))),
            (Value::Bool(a), Value::Bool(b)) => match op {
                "==" => Ok(Value::Bool(a == b)),
                "!=" => Ok(Value::Bool(a != b)),
                _ => Err(format!("unsupported comparison '{}' for bool", op)),
            },
            (a, b) => Err(format!("cannot compare {} and {}", a.type_name(), b.type_name())),
        }
    }

    /// Check if the value is truthy.
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::Str(s) => !s.is_empty(),
            Value::Int(n) => *n != 0,
            Value::Float(f) => *f != 0.0,
            Value::List(items) => !items.is_empty(),
            Value::Map(m) => !m.is_empty(),
        }
    }

    /// Return the type name.
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Str(_) => "str",
            Value::Int(_) => "int",
            Value::Float(_) => "float",
            Value::Bool(_) => "bool",
            Value::List(_) => "list",
            Value::Map(_) => "map",
        }
    }
}

fn int_cmp(a: i64, b: i64, op: &str) -> bool {
    match op {
        "==" => a == b, "!=" => a != b,
        "<" => a < b, ">" => a > b,
        "<=" => a <= b, ">=" => a >= b,
        _ => false,
    }
}

fn float_cmp(a: f64, b: f64, op: &str) -> bool {
    match op {
        "==" => a == b, "!=" => a != b,
        "<" => a < b, ">" => a > b,
        "<=" => a <= b, ">=" => a >= b,
        _ => false,
    }
}

fn str_cmp(a: &str, b: &str, op: &str) -> bool {
    match op {
        "==" => a == b, "!=" => a != b,
        "<" => a < b, ">" => a > b,
        "<=" => a <= b, ">=" => a >= b,
        _ => false,
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_str())
    }
}
