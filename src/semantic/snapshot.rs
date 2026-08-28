use super::RuntimeNodeId;

/// Allocates compact identities while constructing one semantic snapshot.
#[derive(Debug, Default)]
pub struct RuntimeIdAllocator {
    next: u64,
}

impl RuntimeIdAllocator {
    pub fn allocate(&mut self) -> RuntimeNodeId {
        let id = RuntimeNodeId::new(self.next);
        self.next = self.next.saturating_add(1);
        id
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn runtime_ids_are_unique_within_a_snapshot() {
        let mut allocator = RuntimeIdAllocator::default();
        let ids = (0..10_000)
            .map(|_| allocator.allocate())
            .collect::<HashSet<_>>();
        assert_eq!(ids.len(), 10_000);
    }

    #[test]
    fn a_new_snapshot_restarts_runtime_identity() {
        let first = RuntimeIdAllocator::default().allocate();
        let second = RuntimeIdAllocator::default().allocate();
        assert_eq!(first, second);
    }
}
