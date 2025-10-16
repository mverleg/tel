use crate::common::Rev;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::path::PathBuf;

#[derive(Debug)]
pub struct DiskStoreConf {
    path: PathBuf,
    //TODO @mark: cache invalidation policy
}

pub struct DiskStore<E: serde::Serialize + serde::de::DeserializeOwned> {
    conf: DiskStoreConf,
    phantom: PhantomData<E>,
    //TODO @mark: needed? ^
}

impl<E: serde::Serialize + serde::de::DeserializeOwned> DiskStore<E> {

    /// The `rev` should be new; call this only one per rev until clear is called
    pub fn insert(&mut self, rev: Rev, value: &E) {
        let bytes = bincode::serialize(&value).unwrap();
        todo!()
    }

    pub fn clear(&mut self) {
        todo!()
    }
}
