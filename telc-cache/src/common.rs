
#[derive(Debug)]
pub enum Insert<'e, E> {
    Value(Rev, &'e E),
    CacheWipe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rev {
    ix: usize,
}
