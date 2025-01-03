use crate::common::Insert;
use crate::common::Rev;
use crate::disk::DiskStore;
use crate::memory::MemoryStore;
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
        todo!()
    }

    pub fn clear(&mut self) -> (Rev, Option<&E>) {
        todo!()
    }
}
