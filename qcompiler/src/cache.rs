use ::std::fmt::Debug;
use ::std::hash::Hash;

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

//TODO @mark: Mocka exists, but only memory

struct Cache<K, V>
where
    K: Debug + PartialEq + Eq + Hash,
    V: Debug + PartialEq + Eq, {}

impl<K, V> Cache<K, V>
where
    K: Debug + PartialEq + Eq + Hash,
    V: Debug + PartialEq + Eq,
{

}
