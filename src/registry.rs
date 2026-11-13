//! A list of things you refer to by handle, with optional names

use std::{collections::HashMap, fmt, hash::Hash, marker::PhantomData};

/// Refers to one item in a `Registry<T>`. Only a registry can make one. Once its item is removed,
/// the handle stays dead, even if a new item reuses the spot
pub struct Handle<T> {
    index: u32,
    generation: u32,
    _marker: PhantomData<fn() -> T>,
}

impl<T> Handle<T> {
    fn new(index: usize, generation: u32) -> Self {
        Self {
            index: index as u32,
            generation,
            _marker: PhantomData,
        }
    }

    /// A handle for whatever ends up first at `index`, for engine code that fills it in right after
    pub(crate) fn placeholder(index: usize) -> Self {
        Self::new(index, 0)
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
        self.index == other.index && self.generation == other.generation
    }
}
impl<T> Eq for Handle<T> {}
impl<T> Hash for Handle<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.index.hash(state);
        self.generation.hash(state);
    }
}
impl<T> fmt::Debug for Handle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = short_type_name::<T>();
        match self.generation {
            0 => write!(f, "Handle<{name}>({})", self.index),
            g => write!(f, "Handle<{name}>({} v{g})", self.index),
        }
    }
}

fn short_type_name<T>() -> &'static str {
    let full = std::any::type_name::<T>();
    full.rsplit("::").next().unwrap_or(full)
}

struct Slot<T> {
    item: Option<T>,
    /// Bumped every time the item here is removed, so old handles stop matching
    generation: u32,
}

/// A list of things you add and refer to by `Handle`. Names are optional, for looking things up
/// by string
pub struct Registry<T> {
    slots: Vec<Slot<T>>,
    free: Vec<usize>,
    names: HashMap<&'static str, Handle<T>>,
    len: usize,
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
            slots: Vec::new(),
            free: Vec::new(),
            names: HashMap::new(),
            len: 0,
        }
    }

    /// Add an item, optionally under a name, and get its handle back
    pub fn add(&mut self, item: T, name: Option<&'static str>) -> Handle<T> {
        let handle = match self.free.pop() {
            Some(index) => {
                let slot = &mut self.slots[index];
                slot.item = Some(item);
                Handle::new(index, slot.generation)
            }
            None => {
                self.slots.push(Slot {
                    item: Some(item),
                    generation: 0,
                });
                Handle::new(self.slots.len() - 1, 0)
            }
        };
        self.len += 1;
        if let Some(name) = name {
            self.names.insert(name, handle);
        }
        handle
    }

    /// Add an item under `name`, or swap it in for whatever already has that name. Handles to the
    /// old item now refer to the new one
    pub fn add_or_replace(&mut self, item: T, name: &'static str) -> Handle<T> {
        match self.lookup(name) {
            Some(handle) => {
                *self.get_mut(handle).expect("named handles are always live") = item;
                handle
            }
            None => self.add(item, Some(name)),
        }
    }

    /// Take the item out, giving it back. Its handle, and any name it had, stop working
    pub fn remove(&mut self, handle: Handle<T>) -> Option<T> {
        let slot = self.slots.get_mut(handle.index())?;
        if slot.generation != handle.generation {
            return None;
        }
        let item = slot.item.take()?;
        slot.generation += 1;
        self.free.push(handle.index());
        self.names.retain(|_, named| *named != handle);
        self.len -= 1;
        Some(item)
    }

    /// The item `handle` refers to. `None` if it's been removed, or the handle is from a
    /// different registry
    pub fn get(&self, handle: Handle<T>) -> Option<&T> {
        self.slots
            .get(handle.index())
            .filter(|slot| slot.generation == handle.generation)
            .and_then(|slot| slot.item.as_ref())
    }

    /// The item `handle` refers to, to change it
    pub fn get_mut(&mut self, handle: Handle<T>) -> Option<&mut T> {
        self.slots
            .get_mut(handle.index())
            .filter(|slot| slot.generation == handle.generation)
            .and_then(|slot| slot.item.as_mut())
    }

    /// The handle of the item added under `name`
    pub fn lookup(&self, name: &str) -> Option<Handle<T>> {
        self.names.get(name).copied()
    }

    /// How many items there are
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether there are no items
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Every handle and item
    pub fn iter(&self) -> impl Iterator<Item = (Handle<T>, &T)> {
        self.slots.iter().enumerate().filter_map(|(i, slot)| {
            slot.item
                .as_ref()
                .map(|item| (Handle::new(i, slot.generation), item))
        })
    }

    /// Every handle and item, to change them
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (Handle<T>, &mut T)> {
        self.slots.iter_mut().enumerate().filter_map(|(i, slot)| {
            let generation = slot.generation;
            slot.item
                .as_mut()
                .map(|item| (Handle::new(i, generation), item))
        })
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

    #[test]
    fn removing_gives_the_item_back_and_kills_the_handle() {
        let mut reg = Registry::new();
        let a = reg.add("apple", Some("a"));
        assert_eq!(reg.remove(a), Some("apple"));
        assert_eq!(reg.get(a), None);
        assert_eq!(reg.lookup("a"), None);
        assert_eq!(reg.remove(a), None);
        assert!(reg.is_empty());
    }

    #[test]
    fn a_reused_spot_doesnt_answer_to_the_old_handle() {
        let mut reg = Registry::new();
        let old = reg.add("old", None);
        reg.remove(old);
        let new = reg.add("new", None);
        assert_ne!(old, new);
        assert_eq!(reg.get(old), None);
        assert_eq!(reg.get(new), Some(&"new"));
        assert_eq!(reg.iter().count(), 1);
    }
}
