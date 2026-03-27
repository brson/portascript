/// Runtime value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Str(String),
    Int(i64),
    Float(f64),
    Bool(bool),
}

impl Value {
    /// Coerce to string representation.
    pub fn to_str(&self) -> String {
        match self {
            Value::Str(s) => s.clone(),
            Value::Int(n) => n.to_string(),
            Value::Float(f) => f.to_string(),
            Value::Bool(b) => b.to_string(),
        }
    }
}

impl Value {
    /// Apply a binary arithmetic/string operator.
    pub fn binary_op(self, op: &str, other: Value) -> Result<Value, String> {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => match op {
                "+" => Ok(Value::Int(a + b)),
                "-" => Ok(Value::Int(a - b)),
                "*" => Ok(Value::Int(a * b)),
                "/" => {
                    if b == 0 {
                        Err("division by zero".into())
                    } else {
                        Ok(Value::Int(a / b))
                    }
                }
                "%" => {
                    if b == 0 {
                        Err("division by zero".into())
                    } else {
                        Ok(Value::Int(a % b))
                    }
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

    /// Return the type name.
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Str(_) => "str",
            Value::Int(_) => "int",
            Value::Float(_) => "float",
            Value::Bool(_) => "bool",
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_str())
    }
}
