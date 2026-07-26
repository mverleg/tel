# Name mangling / symbol naming

Mangled symbol names are **not a compatibility surface** in Tel: there is no
stable ABI, and a whole program is compiled by one compiler together (see
`20-appendix/04-versioning-and-compatibility.md`). So the scheme can be anything
convenient and may change between compiler versions freely.

## Safety note: hash the full project-root-relative path

When a scheme needs to disambiguate file-private (or otherwise
module-/file-scoped) declarations by hashing a path, hash the **full path from
the project root**, not just the file basename.

Why: Swift hashed *module name + file basename* into its "private discriminator",
which meant two files with the same basename in different directories could not
coexist in one module (Jordan Rose, "Swift Mangling Regret: Private
Discriminators", https://belkadan.com/blog/2021/11/Swift-Mangling-Regret-Private-Discriminators/ ).
Tel does not have the binary-compatibility constraint that made Swift's choice
permanent, so this can never become a frozen mistake — but using the full
root-relative path avoids the same-basename collision in the first place, for
free. Just do it.

(Paths are normalised relative to the project root so the hash is stable across
machines/checkout locations; nothing absolute or machine-specific goes into it.)
