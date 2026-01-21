use crate::common::Insert;
use crate::common::Next;
use crate::common::Rev;
use crate::disk::DiskStore;
use crate::memory::MemoryStore;
use log::info;
use std::ops::Index;

//TODO @mark: maybe add some duplication id, like filename or identifier, because older duplicates may be better to remove

pub struct Store<E: serde::Serialize + serde::de::DeserializeOwned> {
    top: Rev,
    disk: DiskStore<E>,
    memory: MemoryStore<E>,
}

impl <'s, E: serde::Serialize + serde::de::DeserializeOwned> Index<Rev> for &'s Store<E> {
    type Output = Option<&'s E>;

    fn index(&self, _rev: Rev) -> &Self::Output {
        todo!()
    }
}

impl <E: serde::Serialize + serde::de::DeserializeOwned> Store<E> {

    pub fn get(&self, _rev: Rev) -> Option<&E> {
        todo!()
    }

    //TODO @mark: async?
    pub fn set(&mut self, value: E) -> Insert<'_, E> {
        let rev = match self.top.next() {
            Next::Ok(n) => {
                self.top = n;
                n
            },
            Next::Overflow(n) => {
                info!("cache full, index ({} bytes) overflow; clearing!", size_of::<Rev>());
                self.clear();
                self.top = n;
                return Insert::CacheWipe
            },
        };
        let memory_value = self.memory.insert(rev, value);
        self.disk.insert(rev, memory_value);
        Insert::Value(rev, memory_value)
    }

    pub fn clear(&mut self) {
        self.disk.clear();
        self.memory.clear();
    }
}
