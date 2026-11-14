//! A string-keyed map that iterates as a JavaScript object does:
//! array-index keys in ascending order, then the other keys in the order
//! they were first set. table-core keeps column sizing in such an object,
//! and its key order shows wherever the state is read back.

use super::value::array_index;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ObjectMap<V> {
    /// In first-set order.
    entries: Vec<(String, V)>,
}

impl<V: Clone> ObjectMap<V> {
    pub fn new() -> Self {
        ObjectMap {
            entries: Vec::new(),
        }
    }

    /// `object[key]` for an own property.
    pub fn get(&self, key: &str) -> Option<&V> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    /// `object[key] = value`: an existing key keeps its place.
    pub fn insert(&mut self, key: &str, value: V) {
        match self.entries.iter_mut().find(|(k, _)| k == key) {
            Some(entry) => entry.1 = value,
            None => self.entries.push((key.to_owned(), value)),
        }
    }

    /// `Object.entries(object)`.
    pub fn entries(&self) -> Vec<(String, V)> {
        let mut indexed: Vec<(usize, &(String, V))> = self
            .entries
            .iter()
            .filter_map(|entry| array_index(&entry.0).map(|index| (index, entry)))
            .collect();
        indexed.sort_by_key(|(index, _)| *index);
        indexed
            .into_iter()
            .map(|(_, entry)| entry.clone())
            .chain(
                self.entries
                    .iter()
                    .filter(|entry| array_index(&entry.0).is_none())
                    .cloned(),
            )
            .collect()
    }

    /// `Object.assign(target, source)`: copies `source`'s entries in its
    /// iteration order.
    pub fn assign(&mut self, source: &ObjectMap<V>) {
        for (key, value) in source.entries() {
            self.insert(&key, value);
        }
    }

    /// An object built from `[key, value]` pairs, as `Object.fromEntries`.
    pub fn from_entries(entries: impl IntoIterator<Item = (String, V)>) -> Self {
        let mut map = ObjectMap::new();
        for (key, value) in entries {
            map.insert(&key, value);
        }
        map
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_keys_come_first_in_ascending_order() {
        let map = ObjectMap::from_entries([
            ("b".into(), 1),
            ("10".into(), 2),
            ("a".into(), 3),
            ("2".into(), 4),
            ("b".into(), 5),
        ]);
        let keys: Vec<String> = map.entries().into_iter().map(|(k, _)| k).collect();
        assert_eq!(keys, ["2", "10", "b", "a"]);
        assert_eq!(map.get("b"), Some(&5));
    }
}
