use std::collections::HashSet;

use glob_match::glob_match;

pub(crate) fn has_glob(pattern: &str) -> bool {
    pattern
        .bytes()
        .any(|b| matches!(b, b'*' | b'?' | b'[' | b'{' | b'\\' | b'!'))
}

#[derive(Debug, Default, Clone)]
pub(crate) struct Patterns {
    pub literals: HashSet<String>,
    pub globs: Vec<String>,
}

impl Patterns {
    pub fn insert(&mut self, pattern: String) {
        if self.literals.insert(pattern.clone()) && has_glob(&pattern) {
            self.globs.push(pattern);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.literals.is_empty()
    }

    pub fn matches(&self, value: &str) -> bool {
        self.is_empty()
            || self.literals.contains(value)
            || self.globs.iter().any(|p| glob_match(p, value))
    }
}
