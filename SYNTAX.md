
# Steel syntax

The goal of the syntax is to 

* The syntax should be left-recursive with lookahead of 1, to be easy and fast to parse
* The syntax should look terse for small scripts, while expressive enough for medium programs

Some details about it:

* Indentation is not significant, but newlines are; statements must end with ';' and/or a newline
* To split a statement over multiple lines, either break where it can't be closed (e.g. after '[' or '+'), or insert '...'
* Comments start with '#' and are always single-line
* Types are always required on the signatures of public 'things' (enums, structs, functions), otherwise can be omitted if inferrable
* Variables can be declared without any keyword (if immutable), or with "mut" or "local"
* For reasons of performance, simplicity and backwards compatibility, type inference is from expression to result and not exceptionally smart 
* There is a preference for left-to-right style, with some operators having attribute syntax (e.g. `.assert`)
* Closures that take 0 or 1 arguments can be written as just `{...}` anywhere an expression is expected, and can use `it` for the arg
* Closures that take more than 1 argument are written the same as functions, e.g. `fn(a, b) {...}`
* Closures can be placed outside a function invocation, and will be passed as the last positional argument
* Using `self` can be omitted when used, and is not declared as part of functions

