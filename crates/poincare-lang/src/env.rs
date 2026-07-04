//! The runtime environment model: a lexical scope chain mapping names to
//! values.
//!
//! This is generic over the value type `V`. The Phase 3 resolver uses it
//! conceptually for name tracking, and the Phase 4 interpreter will instantiate
//! it with runtime values. Rebinding in the current scope is allowed (`define`
//! overwrites); `assign` updates the nearest existing binding. Surface-level
//! rebinding lowers cleanly onto this model without mutating outer bindings
//! unexpectedly.

use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct Env<V> {
    scopes: Vec<HashMap<String, V>>,
}

impl<V> Default for Env<V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<V> Env<V> {
    /// A new environment with a single (global) scope.
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
        }
    }

    /// Push a fresh inner scope.
    pub fn enter_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    /// Pop the innermost scope. The global scope is never popped.
    pub fn exit_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    /// Number of active scopes (always >= 1).
    pub fn depth(&self) -> usize {
        self.scopes.len()
    }

    /// Bind (or rebind) `name` in the current innermost scope.
    pub fn define(&mut self, name: impl Into<String>, value: V) {
        self.scopes
            .last_mut()
            .expect("environment always has a scope")
            .insert(name.into(), value);
    }

    /// Look up a name from the innermost scope outward.
    pub fn get(&self, name: &str) -> Option<&V> {
        self.scopes.iter().rev().find_map(|scope| scope.get(name))
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut V> {
        self.scopes
            .iter_mut()
            .rev()
            .find_map(|scope| scope.get_mut(name))
    }

    pub fn contains(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    /// Update the nearest existing binding for `name`. Returns `false` if no
    /// binding exists in any scope.
    pub fn assign(&mut self, name: &str, value: V) -> bool {
        match self.get_mut(name) {
            Some(slot) => {
                *slot = value;
                true
            }
            None => false,
        }
    }

    /// Names bound in the current innermost scope.
    pub fn names_in_current_scope(&self) -> impl Iterator<Item = &str> {
        self.scopes
            .last()
            .expect("environment always has a scope")
            .keys()
            .map(String::as_str)
    }
}
