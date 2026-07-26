# Summary

[Introduction](README.md)

# Overview

- [Introduction](01-overview/01-introduction.md)
- [When to Use Tel](01-overview/02-when-to-use-tel.md)
- [Goals and Non-Goals](01-overview/03-goals-and-non-goals.md)
- [A Tour of Tel](01-overview/04-tour.md)
- [Glossary](01-overview/05-glossary.md)
- [Tel for AI-Assisted Development](01-overview/06-tel-for-ai-assisted-development.md)

# Philosophy

- [Priorities and Trade-offs](02-philosophy/01-priorities.md)
- [Maxims of Tel](02-philosophy/02-maxims.md)
- [Features Tel Embraces](02-philosophy/03-features.md)
- [Antifeatures (and Why)](02-philosophy/04-antifeatures.md)

# Lexical Structure

- [Source Encoding](03-lexical-structure/01-source-encoding.md)
- [Tokens](03-lexical-structure/02-tokens.md)
- [Identifiers](03-lexical-structure/03-identifiers.md)
- [Keywords](03-lexical-structure/04-keywords.md)
- [Literals](03-lexical-structure/05-literals.md)
- [Operators and Punctuation](03-lexical-structure/06-operators-and-punctuation.md)
- [Comments](03-lexical-structure/07-comments.md)
- [Whitespace and Newlines](03-lexical-structure/08-whitespace-and-newlines.md)

# Syntax

- [Grammar Notation](04-syntax/01-grammar-notation.md)
- [Expressions vs Statements](04-syntax/02-expressions-vs-statements.md)
- [Blocks](04-syntax/03-blocks.md)
- [Precedence and Associativity](04-syntax/04-precedence-and-associativity.md)
- [Layout Rules](04-syntax/05-layout-rules.md)

# Types

- [Type System Overview](05-types/01-type-system-overview.md)
- [Primitive Types](05-types/02-primitive-types.md)
- [Strings and Text](05-types/03-strings-and-text.md)
- [Tuples and Arrays](05-types/04-tuples-and-arrays.md)
- [Function Types](05-types/05-function-types.md)
- [Option and Nullability](05-types/06-option-and-nullability.md)
- [Generics](05-types/07-generics.md)
- [Type Inference](05-types/08-type-inference.md)
- [Subtyping and Variance](05-types/09-subtyping-and-variance.md)
- [Type Aliases](05-types/10-type-aliases.md)
- [Conversions and Coercions](05-types/11-conversions-and-coercions.md)
- [Refined Types](05-types/12-refined-types.md)
- [Units](05-types/13-units.md)
- [The Never Type](05-types/14-never-type.md)
- [The Record-Shape Calculus](05-types/15-record-shape-calculus.md)

# Bindings and Scope

- [Let Bindings](06-bindings-and-scope/01-let-bindings.md)
- [Mutability](06-bindings-and-scope/02-mutability.md)
- [Constants](06-bindings-and-scope/03-constants.md)
- [Shadowing](06-bindings-and-scope/04-shadowing.md)
- [Scoping Rules](06-bindings-and-scope/05-scoping-rules.md)
- [Destructuring](06-bindings-and-scope/06-destructuring.md)
- [No Global Mutable State](06-bindings-and-scope/07-no-global-mutable-state.md)

# Expressions

- [Literal Expressions](07-expressions/01-literal-expressions.md)
- [Arithmetic and Numeric Operators](07-expressions/02-arithmetic-and-numeric.md)
- [Comparison and Logical Operators](07-expressions/03-comparison-and-logical.md)
- [String Operations](07-expressions/05-string-operations.md)
- [Function Application](07-expressions/06-function-application.md)
- [Field and Index Access](07-expressions/07-field-and-index-access.md)
- [Block Expressions](07-expressions/08-block-expressions.md)
- [Conversion Expressions](07-expressions/09-conversions.md)
- [Pipelines](07-expressions/10-pipelines.md)
- [Fallback Operator](07-expressions/11-fallback-operator.md)

# Control Flow

- [If Expressions](08-control-flow/01-if-expressions.md)
- [Match Expressions](08-control-flow/02-match-expressions.md)
- [While Loops](08-control-flow/03-while-loops.md)
- [For Loops and Iteration](08-control-flow/04-for-loops-and-iteration.md)
- [Loop and Break](08-control-flow/05-loop-and-break.md)
- [Early Return](08-control-flow/06-early-return.md)
- [Error Propagation](08-control-flow/07-error-propagation.md)

# Functions

- [Function Declaration](09-functions/01-function-declaration.md)
- [Parameters and Arguments](09-functions/02-parameters-and-arguments.md)
- [Return Values](09-functions/03-return-values.md)
- [Default and Named Arguments](09-functions/04-default-and-named-arguments.md)
- [Variadic Functions](09-functions/05-variadic-functions.md)
- [Closures and Lambdas](09-functions/06-closures-and-lambdas.md)
- [Higher-Order Functions](09-functions/07-higher-order-functions.md)
- [Method Syntax](09-functions/08-method-syntax.md)
- [Overloading and Dispatch](09-functions/09-overloading-and-dispatch.md)
- [Recursion](09-functions/10-recursion.md)

# Data Modelling

- [Records](10-data-modelling/01-records.md)
- [Union Types](10-data-modelling/02-union-types.md)
- [Traits or Interfaces](10-data-modelling/03-traits-or-interfaces.md)
- [Generic Data Types](10-data-modelling/04-generic-data-types.md)
- [Recursive Types](10-data-modelling/05-recursive-types.md)
- [Pattern Matching In Depth](10-data-modelling/06-pattern-matching-in-depth.md)
- [Equality and Hashing](10-data-modelling/07-equality-and-hashing.md)
- [Ordering](10-data-modelling/08-ordering.md)
- [Collection Types](10-data-modelling/09-collection-types.md)
- [Iterators and Sequences](10-data-modelling/10-iterators-and-sequences.md)

# Dataframes

- [Overview](10a-dataframes/01-overview.md)
- [Table Operations](10a-dataframes/02-table-operations.md)
- [Storage, Mutability, and Evaluation](10a-dataframes/03-storage-mutability-evaluation.md)

# Modules and Crates

- [Modules](11-modules-and-packages/01-modules.md)
- [Imports](11-modules-and-packages/02-imports.md)
- [Visibility](11-modules-and-packages/03-visibility.md)
- [Crates](11-modules-and-packages/04-packages.md)
- [Project Layout](11-modules-and-packages/05-project-layout.md)
- [Module Versioning](11-modules-and-packages/06-versioning.md)
- [Package Manifest](11-modules-and-packages/07-package-manifest.md)
- [Dependency Graph and Locking](11-modules-and-packages/08-dependency-graph-and-locking.md)
- [Package Registry](11-modules-and-packages/09-package-registry.md)
- [Workspaces](11-modules-and-packages/10-workspaces.md)

# Memory and Runtime

- [Value vs Reference Semantics](12-memory-and-runtime/01-value-vs-reference-semantics.md)
- [Stack and Heap](12-memory-and-runtime/02-stack-and-heap.md)
- [Memory Management](12-memory-and-runtime/03-memory-management.md)
- [References and Aliasing](12-memory-and-runtime/04-references-and-aliasing.md)
- [Lifetimes](12-memory-and-runtime/05-lifetimes.md)
- [Runtime Representation](12-memory-and-runtime/06-runtime-representation.md)
- [Allocators](12-memory-and-runtime/07-allocators.md)
- [Substructural Types](12-memory-and-runtime/08-substructural-types.md)

# Error Handling

- [Error-Handling Philosophy](13-error-handling/01-philosophy.md)
- [Result Types](13-error-handling/02-result-types.md)
- [Error Propagation](13-error-handling/03-error-propagation.md)
- [Panics and Aborts](13-error-handling/04-panics-and-aborts.md)
- [Recovery](13-error-handling/05-recovery.md)
- [Fallback Operator](13-error-handling/06-fallback-operator.md)

# Concurrency and Parallelism

- [Overview](14-concurrency-and-parallelism/01-overview.md)
- [Tasks](14-concurrency-and-parallelism/02-tasks.md)
- [Async and Function Colouring](14-concurrency-and-parallelism/03-async-and-function-colouring.md)
- [Structured Concurrency](14-concurrency-and-parallelism/04-structured-concurrency.md)
- [Composing Tasks](14-concurrency-and-parallelism/05-composing-tasks.md)
- [Channels and Message Passing](14-concurrency-and-parallelism/06-channels-and-message-passing.md)
- [Memory Model for Concurrency](14-concurrency-and-parallelism/07-memory-model-for-concurrency.md)
- [Cancellation and Timeouts](14-concurrency-and-parallelism/08-cancellation-and-timeouts.md)
- [Scoped Values](14-concurrency-and-parallelism/09-scoped-values.md)
- [Locks and Concurrency Primitives](14-concurrency-and-parallelism/10-locks-and-concurrency-primitives.md)

# Testing

- [Testing and Benchmarks](14-testing/01-testing.md)

# Metaprogramming

- [Macros](15-metaprogramming/01-macros.md)
- [Reflection](15-metaprogramming/02-reflection.md)
- [Derive and Attributes](15-metaprogramming/03-derive-and-attributes.md)
- [Compile-Time Evaluation](15-metaprogramming/04-compile-time-evaluation.md)
- [What Replaces Macros: Use Cases](15-metaprogramming/05-metaprogramming-use-cases.md)

# FFI and Interop

- [C Interop](16-ffi-and-interop/01-c-interop.md)
- [Calling Conventions and ABI](16-ffi-and-interop/02-calling-conventions-and-abi.md)
- [Binding Other Languages](16-ffi-and-interop/03-binding-other-languages.md)
- [Embedding Tel in a Host Application](16-ffi-and-interop/04-embedding-tel-in-a-host.md)

# Standard Library

- [Standard Library Organisation](17-standard-library/01-stdlib-organisation.md)
- [Platform Layer](17-standard-library/02-platform-layer.md)
- [Prelude](17-standard-library/03-prelude.md)
- [Core Collections](17-standard-library/04-core-collections.md)
- [Iteration and Streams](17-standard-library/05-iteration-and-streams.md)
- [Strings and Text](17-standard-library/06-strings-and-text.md)
- [Numerics and Math](17-standard-library/07-numerics-and-math.md)
- [I/O and Filesystem](17-standard-library/08-io-and-filesystem.md)
- [Time](17-standard-library/09-time.md)
- [OS and Process](17-standard-library/10-os-and-process.md)
- [Networking](17-standard-library/11-networking.md)
- [Concurrency Utilities](17-standard-library/12-concurrency-utilities.md)
- [Data Formats and Serialization](17-standard-library/13-data-formats.md)
- [Observability and Logging](17-standard-library/14-observability-and-logging.md)
- [Randomness, Hashing and Crypto](17-standard-library/15-randomness-hashing-and-crypto.md)
- [Internationalisation](17-standard-library/16-internationalisation.md)
- [Scheduling and Timed Ops](17-standard-library/17-scheduling-and-timed-ops.md)
- [Tel as Data](17-standard-library/18-tel-as-data.md)
- [Testing Utilities](17-standard-library/19-testing-utilities.md)
- [Data Access and ORMs](17-standard-library/20-data-access-and-orms.md)
- [Bitwise and Binary Operations](17-standard-library/21-bitwise-and-binary.md)
- [Compression](17-standard-library/22-compression.md)

# Tooling

- [Compiler](18-tooling/01-compiler.md)
- [Compile Targets](18-tooling/02-compile-targets.md)
- [Build System](18-tooling/03-build-system.md)
- [Package Manager](18-tooling/04-package-manager.md)
- [Formatter](18-tooling/06-formatter.md)
- [Linter](18-tooling/07-linter.md)
- [Debugger](18-tooling/08-debugger.md)
- [Editor Integration](18-tooling/09-editor-integration.md)
- [Documentation Generator](18-tooling/10-documentation-generator.md)
- [Language Cues for the IDE](18-tooling/12-language-cues-for-the-ide.md)

# Use Cases

- [Hello, World](19-use-cases/01-hello-world.md)
- [JSON Schema Validator](19-use-cases/02-json-schema-validator.md)
- [Markup DSL](19-use-cases/03-markup-dsl.md)
- [Spline Interpolation](19-use-cases/04-spline-interpolation.md)
- [Matrix Math and FFT](19-use-cases/05-matrix-and-fft.md)
- [Disruptor Ring Buffer](19-use-cases/06-disruptor-ring-buffer.md)
- [Buffered Thread-Local Generator](19-use-cases/07-buffered-thread-local-generator.md)
- [Linear Resources](19-use-cases/08-linear-resources.md)
- [Entity Identity and Projections](19-use-cases/09-entity-identity-and-projections.md)
- [A Localization Library](19-use-cases/10-localization-library.md)

# Appendix

- [Keywords Reference](20-appendix/01-keywords.md)
- [Operator Reference](20-appendix/02-operator-reference.md)
- [Style Guide](20-appendix/03-style-guide.md)
- [Versioning and Compatibility](20-appendix/04-versioning-and-compatibility.md)
- [Design History and Changelog](20-appendix/05-design-history-and-changelog.md)
- [Deferred Features](20-appendix/06-deferred-features.md)
