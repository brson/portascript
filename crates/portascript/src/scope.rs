use crate::value::Value;
use crate::PsError;

use std::collections::HashMap;

/// A variable entry: value and mutability flag.
struct VarEntry {
    value: Value,
    mutable: bool,
}

/// Block-scoped variable storage.
pub struct Scope {
    frames: Vec<HashMap<String, VarEntry>>,
}

impl Scope {
    pub fn new() -> Self {
        Self {
            frames: vec![HashMap::new()],
        }
    }

    /// Push a new scope frame (entering a block).
    pub fn push(&mut self) {
        self.frames.push(HashMap::new());
    }

    /// Pop the top scope frame (leaving a block).
    pub fn pop(&mut self) {
        self.frames.pop();
    }

    /// Declare a new variable in the current (top) frame.
    pub fn declare(&mut self, name: &str, value: Value, mutable: bool) {
        let frame = self.frames.last_mut().unwrap();
        frame.insert(name.to_string(), VarEntry { value, mutable });
    }

    /// Look up a variable by name, walking frames from top to bottom.
    pub fn get(&self, name: &str) -> Option<&Value> {
        for frame in self.frames.iter().rev() {
            if let Some(entry) = frame.get(name) {
                return Some(&entry.value);
            }
        }
        None
    }

    /// Set an existing variable. Returns error if immutable or not found.
    pub fn set(&mut self, name: &str, value: Value, line: usize, col: usize) -> Result<(), PsError> {
        for frame in self.frames.iter_mut().rev() {
            if let Some(entry) = frame.get_mut(name) {
                if !entry.mutable {
                    return Err(PsError {
                        message: format!("cannot assign to immutable variable '{}'", name),
                        line,
                        col,
                    });
                }
                entry.value = value;
                return Ok(());
            }
        }
        Err(PsError {
            message: format!("undefined variable '{}'", name),
            line,
            col,
        })
    }

    /// Set a value at an index within a mutable variable (map[key] = val).
    pub fn index_set(&mut self, name: &str, index: Value, value: Value, line: usize, col: usize) -> Result<(), PsError> {
        for frame in self.frames.iter_mut().rev() {
            if let Some(entry) = frame.get_mut(name) {
                if !entry.mutable {
                    return Err(PsError {
                        message: format!("cannot assign to immutable variable '{}'", name),
                        line, col,
                    });
                }
                match (&mut entry.value, index) {
                    (Value::Map(m), Value::Str(key)) => {
                        m.insert(key, value);
                        return Ok(());
                    }
                    (Value::List(items), Value::Int(i)) => {
                        let idx = if i < 0 { items.len() as i64 + i } else { i } as usize;
                        if idx < items.len() {
                            items[idx] = value;
                            return Ok(());
                        }
                        return Err(PsError {
                            message: format!("index {} out of bounds (len {})", i, items.len()),
                            line, col,
                        });
                    }
                    _ => {
                        return Err(PsError {
                            message: format!("cannot index-assign to {}", entry.value.type_name()),
                            line, col,
                        });
                    }
                }
            }
        }
        Err(PsError {
            message: format!("undefined variable '{}'", name),
            line, col,
        })
    }
}
