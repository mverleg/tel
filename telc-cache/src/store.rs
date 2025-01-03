use ::std::collections::HashMap;
use ::std::fmt::Debug;
use ::std::ops::Index;
use ::std::path::PathBuf;
use std::marker::PhantomData;

/// Storage on disk and in memory, cleaning up lower utility items when too full.
///
/// * This is a 'cache' (without hashable keys etc), so only for data that can be regenerated.
/// * Items should be the same after de/ser with serde, otherwise disk and memory won't behave the same.
/// * Items are given unique u64 numbers. If this overflows, the cache is wiped.
///   Users should clear all their references, otherwise there may be collisions.
/// * Not optimized for `Copy` types; assumes relatively big data.
///
/// This is just the storage backend; hashing and lookups should happen in `db`.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Ref {
    ix: usize,
}

#[derive(Debug)]
struct DiskStoreConf {
    path: PathBuf,
    //TODO @mark: cache invalidation policy
}

struct DiskStore<E> {
    conf: DiskStoreConf,
    phantom: PhantomData<E>,
    //TODO @mark: needed? ^
}

#[derive(Debug)]
struct MemoryStoreConf {
    //TODO @mark: cache invalidation policy
}

struct MemoryStore<E> {
    conf: MemoryStoreConf,
    data: HashMap<Ref, E>,
}

pub struct Store<E: serde::Serialize + serde::de::DeserializeOwned> {
    top: Ref,
    disk: DiskStore<E>,
    memory: MemoryStore<E>,
}

impl <'s, E: serde::Serialize + serde::de::DeserializeOwned> Index<Ref> for &'s Store<E> {
    type Output = Option<&'s E>;

    fn index(&self, index: Ref) -> &Self::Output {
        self.get(index)
    }
}
impl <E: serde::Serialize + serde::de::DeserializeOwned> Store<E> {

    pub fn get(&self, re: Ref) -> &Option<&E> {
        todo!()
    }

    pub fn set(&mut self, value: E) -> (Ref, Option<&E>) {
        todo!()
    }

    pub fn clear(&mut self) -> (Ref, Option<&E>) {
        todo!()
    }
}
