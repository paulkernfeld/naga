use std::{fmt, hash, marker::PhantomData, num::NonZeroU32};

/// An unique index in the arena array that a handle points to.
///
/// This type is independent of `spirv::Word`. `spirv::Word` is used in data
/// representation. It holds a SPIR-V and refers to that instruction. In
/// structured representation, we use Handle to refer to an SPIR-V instruction.
/// `Index` is an implementation detail to `Handle`.
type Index = NonZeroU32;

/// A strongly typed reference to a SPIR-V element.
#[repr(transparent)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize), serde(into = "SerHandle"))]
#[cfg_attr(feature = "deserialize", derive(serde::Deserialize), serde(from = "SerHandle"))]
pub struct Handle<T> {
    index: Index,
    marker: PhantomData<T>,
}

/// This type allows us to make the serialized representation of a Handle more concise
#[allow(dead_code)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "deserialize", derive(serde::Deserialize))]
enum SerHandle {
    // The single-variant enum makes the serialized RON representation look like `Handle(42)`.
    // Otherwise it would just look like `42`.
    Handle(Index)
}

#[cfg(feature = "serialize")]
impl<T> From<Handle<T>> for SerHandle {
    fn from(handle: Handle<T>) -> Self {
        SerHandle::Handle(handle.index)
    }
}

#[cfg(feature = "deserialize")]
impl<T> From<SerHandle> for Handle<T> {
    fn from(handle: SerHandle) -> Self {
        match handle {
            SerHandle::Handle(index) => Handle { index, marker: PhantomData },
        }
    }
}

impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        Handle {
            index: self.index,
            marker: self.marker,
        }
    }
}
impl<T> Copy for Handle<T> {}
impl<T> PartialEq for Handle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
    }
}
impl<T> Eq for Handle<T> {}
impl<T> fmt::Debug for Handle<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "Handle({})", self.index)
    }
}
impl<T> hash::Hash for Handle<T> {
    fn hash<H: hash::Hasher>(&self, hasher: &mut H) {
        self.index.hash(hasher)
    }
}

impl<T> Handle<T> {
    #[cfg(test)]
    pub const DUMMY: Self = Handle {
        index: unsafe { NonZeroU32::new_unchecked(!0) },
        marker: PhantomData,
    };

    pub(crate) fn new(index: Index) -> Self {
        Handle {
            index,
            marker: PhantomData,
        }
    }

    /// Returns the zero-based index of this handle.
    pub fn index(self) -> usize {
        let index = self.index.get() - 1;
        index as usize
    }
}

/// An arena holding some kind of component (e.g., type, constant,
/// instruction, etc.) that can be referenced.
#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "deserialize", derive(serde::Deserialize))]
pub struct Arena<T> {
    /// Values of this arena.
    data: Vec<T>,
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Arena<T> {
    pub fn new() -> Self {
        Arena { data: Vec::new() }
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = (Handle<T>, &T)> {
        self.data.iter().enumerate().map(|(i, v)| {
            let position = i + 1;
            let index = unsafe { Index::new_unchecked(position as u32) };
            (Handle::new(index), v)
        })
    }

    /// Adds a new value to the arena, returning a typed handle.
    ///
    /// The value is not linked to any SPIR-V module.
    pub fn append(&mut self, value: T) -> Handle<T> {
        let position = self.data.len() + 1;
        let index = unsafe { Index::new_unchecked(position as u32) };
        self.data.push(value);
        Handle::new(index)
    }

    /// Adds a value with a check for uniqueness: returns a handle pointing to
    /// an existing element if its value matches the given one, or adds a new
    /// element otherwise.
    pub fn fetch_or_append(&mut self, value: T) -> Handle<T>
    where
        T: PartialEq,
    {
        if let Some(index) = self.data.iter().position(|d| d == &value) {
            let index = unsafe { Index::new_unchecked((index + 1) as u32) };
            Handle::new(index)
        } else {
            self.append(value)
        }
    }
}

impl<T> std::ops::Index<Handle<T>> for Arena<T> {
    type Output = T;
    fn index(&self, handle: Handle<T>) -> &T {
        let index = handle.index.get() - 1;
        &self.data[index as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_non_unique() {
        let mut arena: Arena<u8> = Arena::new();
        let t1 = arena.append(0);
        let t2 = arena.append(0);
        assert!(t1 != t2);
        assert!(arena[t1] == arena[t2]);
    }

    #[test]
    fn append_unique() {
        let mut arena: Arena<u8> = Arena::new();
        let t1 = arena.append(0);
        let t2 = arena.append(1);
        assert!(t1 != t2);
        assert!(arena[t1] != arena[t2]);
    }

    #[test]
    fn fetch_or_append_non_unique() {
        let mut arena: Arena<u8> = Arena::new();
        let t1 = arena.fetch_or_append(0);
        let t2 = arena.fetch_or_append(0);
        assert!(t1 == t2);
        assert!(arena[t1] == arena[t2])
    }

    #[test]
    fn fetch_or_append_unique() {
        let mut arena: Arena<u8> = Arena::new();
        let t1 = arena.fetch_or_append(0);
        let t2 = arena.fetch_or_append(1);
        assert!(t1 != t2);
        assert!(arena[t1] != arena[t2]);
    }

    #[test]
    #[cfg(feature = "serialize")]
    fn handle_ser() {
        let handle_ser = ron::ser::to_string(&Handle::<()>::DUMMY).unwrap();
        assert_eq!(handle_ser, "Handle(4294967295)");
    }

    #[test]
    #[cfg(feature = "deserialize")]
    fn handle_de() {
        type TestHandle = Handle<()>;
        let handle_de: TestHandle = ron::de::from_str("Handle(4294967295)").unwrap();
        assert_eq!(handle_de.index, TestHandle::DUMMY.index);
    }

    #[test]
    #[cfg(all(feature = "serialize", feature = "deserialize"))]
    fn handle_ser_de() {
        type TestHandle = Handle<()>;
        let handle_ser = ron::ser::to_string(&TestHandle::DUMMY).unwrap();
        let handle_ser_de: TestHandle = ron::de::from_str(&handle_ser).unwrap();
        assert_eq!(handle_ser_de.index, TestHandle::DUMMY.index);
    }
}
