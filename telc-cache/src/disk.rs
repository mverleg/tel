use ::std::fmt::Debug;
use ::std::marker::PhantomData;
use ::std::path::PathBuf;

#[derive(Debug)]
pub struct DiskStoreConf {
    path: PathBuf,
    //TODO @mark: cache invalidation policy
}

pub struct DiskStore<E> {
    conf: DiskStoreConf,
    phantom: PhantomData<E>,
    //TODO @mark: needed? ^
}
