use either::Either;
use ::std::fmt::Debug;
use ::std::hash::Hash;
use std::ops::Index;

/// There are a lot of requirements for the cache:
///
/// * Values can be missing, present or being computed; in the last case they can be (async) awaited
/// * Reads are very common, additions and updates medium, deletes are rare and in batches
/// * There is a memory cache and a disk cache; disk is bigger but not necessarily superset
/// * Caches should be thread safe. Disk access may be made exclusive.
///
/// There are also some expectations on the data:
///
/// * Good and fast hash codes and equals for keys, equals for values
/// * Keys and values serde de/serializable (for disk)
/// * Simultaneous immutable borrows of different keys can exist
//TODO @mark: ^ let's start with memory only for the first version

//TODO @mark: Mocka exists, but only memory

type Ix = u32;
const _: () = assert!(Ix::MAX as u128 <= std::usize::MAX as u128, "Ix type is bigger than usize, this is useless");
const _: () = assert_eq!(Ix::MIN, 0, "Ix allows negative values, this is a waste of bytes");

struct Id<K, V> {
    ix: Ix,
}

struct Db<K, V>
where
    K: Debug + PartialEq + Eq + Hash,
    V: Debug + PartialEq + Eq, {}

impl<K, V> Db<K, V>
where
    K: Debug + PartialEq + Eq + Hash,
    V: Debug + PartialEq + Eq,
{
    //TODO @mark: should Option be default? or is peek rare and usually we use claim?
    fn peek(&self, key: &K) -> Option<Res<&V>> {
        todo!()
    }

    fn claim(&mut self, key: K) -> Either<Vacancy<V>, &V> {
        todo!()
    }
}

//TODO @mark: impl Index<Ix<_>> for Db {}

struct Res<V>
where
    V: Debug + PartialEq + Eq, {}

impl<V> Res<V>
where
    V: Debug + PartialEq + Eq,
{
    //TODO @mark: async method, 'block' until ready
    fn get(&self) -> &V {
        todo!()
    }
}

struct Vacancy<V>
where
    V: Debug + PartialEq + Eq, {}

impl<V> Vacancy<V>
where
    V: Debug + PartialEq + Eq,
{
    fn insert(self, value: V) -> V {
        todo!()
    }
}
