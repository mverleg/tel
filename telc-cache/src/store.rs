use crate::common::Rev;
use crate::common::Insert;
use crate::common::Next;
use crate::disk::DiskStore;
use crate::memory::MemoryStore;
use ::log::info;
use ::std::ops::Index;

pub struct Store<E: serde::Serialize + serde::de::DeserializeOwned> {
    top: Rev,
    disk: DiskStore<E>,
    memory: MemoryStore<E>,
}

impl <'s, E: serde::Serialize + serde::de::DeserializeOwned> Index<Rev> for &'s Store<E> {
    type Output = Option<&'s E>;

    fn index(&self, rev: Rev) -> &Self::Output {
        todo!()
    }
}

impl <E: serde::Serialize + serde::de::DeserializeOwned> Store<E> {

    pub fn get(&self, rev: Rev) -> Option<&E> {
        todo!()
    }

    pub fn set(&mut self, value: E) -> Insert<E> {
        match self.top.next() {
            Next::Ok(n) => self.top = n,
            Next::Overflow(n) => {
                info!("cache full, index ({} bytes) overflow; clearing!", size_of::<Rev>());
                self.clear();
                self.top = n;
                return Insert::CacheWipe
            },
        }
        ()
    }

    pub fn clear(&mut self) -> (Rev, Option<&E>) {
        todo!()
    }
}
