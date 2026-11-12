//! A list of things you refer to by handle, with optional names

use std::{collections::HashMap, fmt, hash::Hash, marker::PhantomData};

/// Refers to one item in a `Registry<T>`. Only a registry can make one, so it always came from
/// adding something
pub struct Handle<T> {
    index: u32,
    _marker: PhantomData<fn() -> T>,
}

impl<T> Handle<T> {
    fn new(index: usize) -> Self {
        Self {
            index: index as u32,
            _marker: PhantomData,
        }
    }

    /// A handle for whatever ends up at `index`, for engine code that fills it in right after
    pub(crate) fn placeholder(index: usize) -> Self {
        Self::new(index)
    }

    fn index(self) -> usize {
        self.index as usize
    }
}

// Written out by hand so handles are Copy and comparable even when `T` isn't
impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Handle<T> {}
impl<T> PartialEq for Handle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
    }
}
impl<T> Eq for Handle<T> {}
impl<T> Hash for Handle<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.index.hash(state);
    }
}
impl<T> fmt::Debug for Handle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Handle<{}>({})", short_type_name::<T>(), self.index)
    }
}

fn short_type_name<T>() -> &'static str {
    let full = std::any::type_name::<T>();
    full.rsplit("::").next().unwrap_or(full)
}

/// A list of things you add once and refer to by `Handle`. Names are optional, for looking
/// things up by string
pub struct Registry<T> {
    items: Vec<T>,
    names: HashMap<&'static str, Handle<T>>,
}

impl<T> Default for Registry<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Registry<T> {
    /// An empty registry
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            names: HashMap::new(),
        }
    }

    /// Add an item, optionally under a name, and get its handle back
    pub fn add(&mut self, item: T, name: Option<&'static str>) -> Handle<T> {
        let handle = Handle::new(self.items.len());
        self.items.push(item);
        if let Some(name) = name {
            self.names.insert(name, handle);
        }
        handle
    }

    /// Add an item under `name`, or swap it in for whatever already has that name. Handles to the
    /// old item now refer to the new one
    pub fn add_or_replace(&mut self, item: T, name: &'static str) -> Handle<T> {
        match self.names.get(name).copied() {
            Some(handle) => {
                self.items[handle.index()] = item;
                handle
            }
            None => self.add(item, Some(name)),
        }
    }

    /// The item `handle` refers to. Only `None` for a handle from a different registry
    pub fn get(&self, handle: Handle<T>) -> Option<&T> {
        self.items.get(handle.index())
    }

    /// The item `handle` refers to, to change it
    pub fn get_mut(&mut self, handle: Handle<T>) -> Option<&mut T> {
        self.items.get_mut(handle.index())
    }

    /// The handle of the item added under `name`
    pub fn lookup(&self, name: &str) -> Option<Handle<T>> {
        self.names.get(name).copied()
    }

    /// How many items there are
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether there are no items
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Every handle and item, in the order they were added
    pub fn iter(&self) -> impl Iterator<Item = (Handle<T>, &T)> {
        self.items
            .iter()
            .enumerate()
            .map(|(i, item)| (Handle::new(i), item))
    }

    /// Every handle and item, to change them
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (Handle<T>, &mut T)> {
        self.items
            .iter_mut()
            .enumerate()
            .map(|(i, item)| (Handle::new(i), item))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_find_what_was_added() {
        let mut reg = Registry::new();
        let a = reg.add("apple", Some("a"));
        let b = reg.add("banana", None);
        assert_eq!(reg.get(a), Some(&"apple"));
        assert_eq!(reg.get(b), Some(&"banana"));
        assert_eq!(reg.lookup("a"), Some(a));
        assert_eq!(reg.lookup("b"), None);
        assert_ne!(a, b);
    }

    #[test]
    fn replacing_by_name_keeps_the_handle() {
        let mut reg = Registry::new();
        let first = reg.add_or_replace(1, "x");
        let second = reg.add_or_replace(2, "x");
        assert_eq!(first, second);
        assert_eq!(reg.get(first), Some(&2));
        assert_eq!(reg.len(), 1);
    }
}
