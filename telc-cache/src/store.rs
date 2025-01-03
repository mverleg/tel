use crate::disk::DiskStore;
use crate::memory::MemoryStore;
use crate::rev::Rev;
use ::std::ops::Index;

pub struct Store<E: serde::Serialize + serde::de::DeserializeOwned> {
    top: Rev,
    disk: DiskStore<E>,
    memory: MemoryStore<E>,
}

impl <'s, E: serde::Serialize + serde::de::DeserializeOwned> Index<Rev> for &'s Store<E> {
    type Output = Option<&'s E>;

    fn index(&self, index: Rev) -> &Self::Output {
        self.get(index)
    }
}
impl <E: serde::Serialize + serde::de::DeserializeOwned> Store<E> {

    pub fn get(&self, re: Rev) -> &Option<&E> {
        todo!()
    }

    pub fn set(&mut self, value: E) -> (Rev, &E) {
        todo!()
    }

    pub fn clear(&mut self) -> (Rev, Option<&E>) {
        todo!()
    }
}
