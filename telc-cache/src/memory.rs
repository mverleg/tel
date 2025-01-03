use ::std::collections::HashMap;
use ::std::fmt::Debug;
use crate::rev::Rev;

#[derive(Debug)]
pub struct MemoryStoreConf {
    //TODO @mark: cache invalidation policy
}

pub struct MemoryStore<E> {
    conf: MemoryStoreConf,
    data: HashMap<Rev, E>,
}
