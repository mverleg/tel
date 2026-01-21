use std::collections::HashSet;
use std::fmt;
use std::sync::LazyLock;

use crate::SString;
use log::debug;
use serde::Serialize;
use serde::Serializer;

//TODO @mark: should actively-used keywords be in here?
//TODO @mark: should 'assert' be reserved?
const RESERVED: [&str; 138] = [
    "abstract",
    "alias",
    "all",
    "annotation",
    "any",
    "as",
    "async",
    "auto",
    "await",
    "become",
    "bool",
    "box",
    "break",
    "by",
    "byte",
    "catch",
    "class",
    "closed",
    "companion",
    "const",
    "constructor",
    "continue",
    "data",
    "debug",
    "def",
    "default",
    "defer",
    "del",
    "delegate",
    "delegates",
    "delete",
    "derive",
    "deriving",
    "do",
    "double",
    "dynamic",
    "elementwise",
    "end",
    "eval",
    "except",
    "extends",
    "extern",
    "family",
    "field",
    "final",
    "finally",
    "float",
    "fun",
    "for",
    "get",
    "global",
    "goto",
    "impl",
    "implements",
    "import",
    "in",
    "init",
    "inject",
    "int",
    "interface",
    "internal",
    "intersect",
    "intersection",
    "io",
    "is",
    "lambda",
    "lateinit",
    "lazy",
    "let",
    "local",
    "loop",
    "macro",
    "main",
    "match",
    "module",
    "move",
    "NaN",
    "native",
    "nill",
    "none",
    "null",
    "object",
    "open",
    "operator",
    "out",
    "outer",
    "override",
    "package",
    "param",
    "pass",
    "private",
    "proof",
    "public",
    "pure",
    "raise",
    "real",
    "rec",
    "reified",
    "return",
    "sealed",
    "select",
    "self",
    "set",
    "sizeof",
    "spawn",
    "static",
    "steel",
    "super",
    "switch",
    "sync",
    "synchronized",
    "tailrec",
    "task",
    "tel",
    "test",
    "this",
    "throw",
    "throws",
    "to",
    "trait",
    "transient",
    "try",
    "type",
    "union",
    "unite",
    "unsafe",
    "until",
    "use",
    "val",
    "var",
    "vararg",
    "virtual",
    "volatile",
    "when",
    "where",
    "while",
    "xor",
    "yield",
];

static RESERVED_SET: LazyLock<HashSet<&'static str>> = LazyLock::new(|| RESERVED.into_iter().collect());


#[derive(Debug, Clone, PartialEq, Eq)]
//TODO @mark: serialize as string
pub struct Identifier {
    name: SString,
}

impl Serialize for Identifier {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer,
    {
        serializer.serialize_str(&self.name)
    }
}

#[derive(Debug)]
pub enum IdentifierErr { TooShort(SString), InvalidSymbol(SString, char), Reserved(&'static str) }

impl Identifier {
    pub fn new(name: impl Into<SString>) -> Result<Self, IdentifierErr> {
        // [a-zA-Z][a-zA-Z0-9_]*
        let name = name.into();
        for ch in name.chars() {
            match ch {
                '0'..='9' => {}
                'a'..='z' => {}
                'A'..='Z' => {}
                '_' => {}
                unexpected => {
                    debug!("reject identifier because '{name}' contains '{unexpected}'");
                    return Err(IdentifierErr::InvalidSymbol(name, unexpected));
                }
            }
        }
        let Some(first) = name.chars().next() else {
            return Err(IdentifierErr::TooShort(name))
        };
        match first {
            'a'..='z' => {}
            'A'..='Z' => {}
            '_' => {}
            //TODO @mark: allow _ as leading char?
            unexpected => {
                debug!("reject identifier because '{name}' starts with '{unexpected}'");
                return Err(IdentifierErr::InvalidSymbol(name, unexpected));
            }
        }
        if let Some(res) = RESERVED_SET.get(name.as_str()) {
            debug!("reject identifier because '{name}' is reserved",);
            return Err(IdentifierErr::Reserved(res));
        }
        Ok(Identifier { name })
    }
}

impl fmt::Display for Identifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}