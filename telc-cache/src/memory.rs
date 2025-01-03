use ::std::collections::HashMap;
use ::std::fmt::Debug;
use std::collections::hash_map::Entry;
use crate::common::Rev;

#[derive(Debug)]
pub struct MemoryStoreConf {
    //TODO @mark: cache invalidation policy
}

pub struct MemoryStore<E> {
    conf: MemoryStoreConf,
    data: HashMap<Rev, E>,
}

impl <E> MemoryStore<E> {

    /// The `rev` should be new; call this only one per rev until clear is called
    pub fn insert(&mut self, rev: Rev, value: E) -> &E {
        match self.data.entry(rev) {
            Entry::Occupied(_) => panic!("same rev was .insert()'ed more than once without .clear(): {rev:?}"),
            Entry::Vacant(vacancy) => {
                vacancy.insert(value)
            }
        }
    }

    pub fn clear(&mut self) {
        self.data.clear();
        self.data.shrink_to_fit();
    }
}