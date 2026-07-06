use std::collections::HashMap;
use std::fmt;
use std::sync::Mutex;
use serde::Deserialize;
use serde::Serialize;

/// An interned string, represented as a small index. `Copy`, and hashing /
/// equality are trivial integer ops. Resolve back to the string via [`Interner`].
///
/// Note: the index is process-local. It serializes transparently as the raw
/// index, which is only meaningful within the run that produced it; portable
/// (string-based) serialization is handled at the cache boundary, not here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Sym(u32);

/// Interns strings into [`Sym`] indices and resolves them back.
///
/// Interning is done through `&self` (interior mutability) so it can be shared
/// as `&Interner` across the concurrent compile. Interned strings are leaked to
/// `'static`, which is fine here: the compiler already leaks its `Global`, and
/// interned identifiers live for the whole run.
pub struct Interner {
    inner: Mutex<InternerInner>,
}

struct InternerInner {
    strings: Vec<&'static str>,
    lookup: HashMap<&'static str, Sym>,
}

impl Interner {
    pub fn new() -> Interner {
        Interner {
            inner: Mutex::new(InternerInner {
                strings: Vec::with_capacity(256),
                lookup: HashMap::with_capacity(256),
            }),
        }
    }

    pub fn intern(&self, s: &str) -> Sym {
        let mut g = self.inner.lock().unwrap();
        if let Some(&sym) = g.lookup.get(s) {
            return sym;
        }
        let leaked: &'static str = Box::leak(s.to_owned().into_boxed_str());
        let sym = Sym(g.strings.len() as u32);
        g.strings.push(leaked);
        g.lookup.insert(leaked, sym);
        sym
    }

    /// Resolve a `Sym` back to its string. The returned reference is `'static`
    /// (interned strings are never freed), so it outlives the lock guard.
    pub fn resolve(&self, sym: Sym) -> &'static str {
        self.inner.lock().unwrap().strings[sym.0 as usize]
    }
}

impl Default for Interner {
    fn default() -> Self {
        Interner::new()
    }
}

/// Display adapter that carries the [`Interner`] needed to render interned data.
///
/// This is the single generic wrapper for the whole codebase: types implement
/// [`CtxDisplay`] and recurse by passing `i` down, wrapping children in `Ctx`
/// only where a `Display` is required (e.g. `write!`). Usage:
/// `write!(f, "{}", Ctx(&step, interner))`.
pub struct Ctx<'a, T: ?Sized>(pub &'a T, pub &'a Interner);

impl<T: CtxDisplay + ?Sized> fmt::Display for Ctx<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f, self.1)
    }
}

/// Like [`fmt::Display`], but with access to the [`Interner`] so interned data
/// can be rendered as its original string.
pub trait CtxDisplay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>, i: &Interner) -> fmt::Result;
}

impl CtxDisplay for Sym {
    fn fmt(&self, f: &mut fmt::Formatter<'_>, i: &Interner) -> fmt::Result {
        f.write_str(i.resolve(*self))
    }
}

/// A simple (identifier) name, e.g. a variable or function name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Name {
    sym: Sym,
}

impl Name {
    pub fn intern(interner: &Interner, name: &str) -> Name {
        Name { sym: interner.intern(name) }
    }

    /// Wrap an already-interned symbol. The caller must guarantee `sym` came
    /// from the live interner — the only sanctioned user is the portable
    /// reader (src/portable.rs), which re-interns a stored string table.
    pub(crate) fn from_sym(sym: Sym) -> Name {
        Name { sym }
    }

    pub fn sym(&self) -> Sym {
        self.sym
    }

    pub fn resolve(&self, interner: &Interner) -> &'static str {
        interner.resolve(self.sym)
    }
}

impl CtxDisplay for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>, i: &Interner) -> fmt::Result {
        self.sym.fmt(f, i)
    }
}

/// A file path, interned like everything else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Path {
    sym: Sym,
}

impl Path {
    pub fn intern(interner: &Interner, path: &str) -> Path {
        Path { sym: interner.intern(path) }
    }

    /// Wrap an already-interned symbol — see [`Name::from_sym`].
    pub(crate) fn from_sym(sym: Sym) -> Path {
        Path { sym }
    }

    pub fn sym(&self) -> Sym {
        self.sym
    }

    pub fn resolve(&self, interner: &Interner) -> &'static str {
        interner.resolve(self.sym)
    }
}

impl CtxDisplay for Path {
    fn fmt(&self, f: &mut fmt::Formatter<'_>, i: &Interner) -> fmt::Result {
        self.sym.fmt(f, i)
    }
}

/// A fully-qualified location: a file path plus a name within it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FQ {
    path: Path,
    name: Name,
}

impl FQ {
    pub fn intern(interner: &Interner, path: &str, name: &str) -> FQ {
        FQ {
            path: Path::intern(interner, path),
            name: Name::intern(interner, name),
        }
    }

    /// Assemble from already-interned parts — see [`Name::from_sym`].
    pub(crate) fn from_parts(path: Path, name: Name) -> FQ {
        FQ { path, name }
    }

    pub fn path(&self) -> Path {
        self.path
    }

    pub fn name(&self) -> Name {
        self.name
    }

    pub fn path_str(&self, interner: &Interner) -> &'static str {
        self.path.resolve(interner)
    }

    pub fn name_str(&self, interner: &Interner) -> &'static str {
        self.name.resolve(interner)
    }
}

impl CtxDisplay for FQ {
    fn fmt(&self, f: &mut fmt::Formatter<'_>, i: &Interner) -> fmt::Result {
        write!(f, "{}::{}", Ctx(&self.path, i), Ctx(&self.name, i))
    }
}
