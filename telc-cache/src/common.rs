
#[derive(Debug)]
pub enum Insert<'e, E> {
    Value(Rev, &'e E),
    CacheWipe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rev {
    ix: usize,
}

#[derive(Debug)]
pub enum Next {
    Ok(Rev),
    Overflow(Rev),
}

impl Rev {
    pub(crate) fn next(self) -> Next {
        match self.ix.checked_add(1) {
            Some(n) => Next::Ok(Self { ix: n }),
            None => Next::Overflow(Self { ix: 0 }),
        }
    }
}
