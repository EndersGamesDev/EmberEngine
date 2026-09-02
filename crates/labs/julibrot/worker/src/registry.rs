//! Generation-checked compact orbit registry seam for the app.

/// Registry lookup or identifier failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegistryError {
    /// Identifier zero or an unoccupied identifier was requested.
    Missing,
    /// Identifier exists but belongs to another orbit generation.
    StaleGeneration,
    /// No additional nonzero `u32` identifier can be represented.
    IdentifierExhausted,
}

#[derive(Debug)]
struct Entry<T> {
    generation: u32,
    value: T,
}

/// Dense session-local mapping from compact handles to downstream orbit values.
#[derive(Debug, Default)]
pub struct OrbitRegistry<T> {
    entries: Vec<Option<Entry<T>>>,
}

impl<T> OrbitRegistry<T> {
    /// Creates an empty registry without reserving an identifier.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Inserts a value and returns its nonzero generation-tagged handle.
    ///
    /// # Errors
    ///
    /// Returns `IdentifierExhausted` if another `u32` identifier cannot be made.
    pub fn insert(
        &mut self,
        generation: u32,
        value: T,
    ) -> Result<crate::OrbitHandle, RegistryError> {
        if let Some(index) = self.entries.iter().position(Option::is_none) {
            self.entries[index] = Some(Entry { generation, value });
            return handle_for(index, generation);
        }
        let index = self.entries.len();
        let handle = handle_for(index, generation)?;
        self.entries.push(Some(Entry { generation, value }));
        Ok(handle)
    }

    /// Borrows a value only when both identifier and generation match.
    ///
    /// # Errors
    ///
    /// Returns `Missing` for an absent ID and `StaleGeneration` for a mismatch.
    pub fn get(&self, handle: crate::OrbitHandle) -> Result<&T, RegistryError> {
        let entry = self.entry(handle)?;
        Ok(&entry.value)
    }

    /// Removes a value only when both identifier and generation match.
    ///
    /// # Errors
    ///
    /// Returns `Missing` for an absent ID and `StaleGeneration` for a mismatch.
    pub fn remove(&mut self, handle: crate::OrbitHandle) -> Result<T, RegistryError> {
        self.entry(handle)?;
        let index = handle_index(handle)?;
        let entry = self.entries[index].take().ok_or(RegistryError::Missing)?;
        Ok(entry.value)
    }

    fn entry(&self, handle: crate::OrbitHandle) -> Result<&Entry<T>, RegistryError> {
        let index = handle_index(handle)?;
        let entry = self
            .entries
            .get(index)
            .and_then(Option::as_ref)
            .ok_or(RegistryError::Missing)?;
        if entry.generation != handle.generation {
            return Err(RegistryError::StaleGeneration);
        }
        Ok(entry)
    }
}

fn handle_index(handle: crate::OrbitHandle) -> Result<usize, RegistryError> {
    let nonzero = handle.id.checked_sub(1).ok_or(RegistryError::Missing)?;
    let index = usize::try_from(nonzero).map_err(|_| RegistryError::Missing)?;
    Ok(index)
}

fn handle_for(index: usize, generation: u32) -> Result<crate::OrbitHandle, RegistryError> {
    let id = index
        .checked_add(1)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or(RegistryError::IdentifierExhausted)?;
    Ok(crate::OrbitHandle { id, generation })
}

#[cfg(test)]
mod tests {
    use super::{OrbitRegistry, RegistryError};
    use crate::OrbitHandle;

    #[test]
    fn registry_rejects_generation_mismatch_and_reuses_compact_ids() {
        let mut registry = OrbitRegistry::new();
        let first = registry.insert(7, "first").unwrap();
        assert_eq!(first, OrbitHandle { id: 1, generation: 7 });
        assert_eq!(registry.get(first), Ok(&"first"));
        assert_eq!(
            registry.get(OrbitHandle { generation: 8, ..first }),
            Err(RegistryError::StaleGeneration)
        );
        assert_eq!(registry.remove(first), Ok("first"));
        let reused = registry.insert(9, "second").unwrap();
        assert_eq!(reused, OrbitHandle { id: 1, generation: 9 });
        assert_eq!(registry.get(first), Err(RegistryError::StaleGeneration));
    }
}
