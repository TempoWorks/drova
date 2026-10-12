use glob_match::glob_match;
use indexmap::IndexMap;

use crate::patterns::has_glob;

pub(crate) struct Registry<'a, T: ?Sized> {
    entries: IndexMap<String, &'a T>,
    globs: Vec<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_matches_win_and_glob_replacement_preserves_priority() {
        let mut registry = Registry::<str>::default();
        for id in 0..1000 {
            registry.insert(format!("exact/{id}"), "unrelated");
        }
        registry.insert("text/*".into(), "first");
        registry.insert("*".into(), "second");
        registry.insert("text/plain".into(), "exact");
        registry.insert("text/*".into(), "replacement");
        assert_eq!(registry.matching("text/plain"), Some("exact"));
        assert_eq!(registry.matching("text/html"), Some("replacement"));
        assert_eq!(registry.exact("text/html"), None);
        assert_eq!(registry.globs.len(), 2);
        assert_eq!(registry.keys().count(), 1003);
    }
}

impl<T: ?Sized> Default for Registry<'_, T> {
    fn default() -> Self {
        Self {
            entries: IndexMap::new(),
            globs: Vec::new(),
        }
    }
}

impl<'a, T: ?Sized> Registry<'a, T> {
    pub fn insert(&mut self, key: String, value: &'a T) {
        let new_glob = !self.entries.contains_key(&key) && has_glob(&key);
        let (index, _) = self.entries.insert_full(key, value);
        if new_glob {
            self.globs.push(index);
        }
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }

    pub fn exact(&self, key: &str) -> Option<&'a T> {
        self.entries.get(key).copied()
    }

    pub fn matching(&self, key: &str) -> Option<&'a T> {
        self.exact(key).or_else(|| {
            self.globs.iter().find_map(|&index| {
                let (pattern, handler) = self.entries.get_index(index)?;
                glob_match(pattern, key).then_some(*handler)
            })
        })
    }
}
