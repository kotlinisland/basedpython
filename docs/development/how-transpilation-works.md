# how transpilation works

basedpython transpiles `.by` source files into standard Python

## pipeline

```
source (.by)
  │
  ├─ 1. parse (ruff_python_parser)
  │     └─ produces a Python AST
  │
  ├─ 2. build symbol table
  │     └─ collects module-level names for use by transforms
  │
  ├─ 3. run transforms
  │     └─ each transform walks the AST and emits text edits (range → replacement)
  │
  ├─ 4. collect & deduplicate edits
  │     └─ overlapping edits are resolved
  │
  ├─ 5. apply edits to source text
  │     └─ produces the output Python string
  │
  ├─ 6. append preamble
  │     └─ generated imports (typing, typing_extensions, etc.) are prepended
  │
  └─ 7. build source map
        └─ maps output byte offsets back to input byte offsets
```

## transforms

each transform is a struct that implements ruff's `Visitor` trait. it walks the AST looking for patterns it handles and records text edits as `(TextRange, String)` pairs. transforms do not modify the AST — they only produce edits against the original source text

all transforms run independently over the same AST in a single pass:

| transform | file | what it does |
|---|---|---|
| subscription normalization | `subscript.rs` | normalizes tuple subscript keys |
| mutable defaults | `mutable_defaults.rs` | rewrites mutable default arguments to sentinel pattern |
| typing redirect | `typing_redirect.rs` | redirects `typing` imports to `typing_extensions` |
| generics polyfill | `generics.rs` | desugars PEP 695 generics to TypeVar/Generic |
| compat rewrites | `compat.rs` | rewrites expressions for older Python versions |
| tuple literal types | `annotation.rs` | `(int, str)` → `tuple[int, str]` in annotations |
| literal types | `literal_types.rs` | string/number literals in type positions → `Literal[...]` |
| auto-quoting | `auto_quote.rs` | quotes forward self-references in class definitions |
| intersection types | `intersection.rs` | `A & B` → `Intersection[A, B]` |
| callable syntax | `callable.rs` | `(int) -> int` → `Callable[[int], int]` |
| unpack syntax | `unpack.rs` | `*tuple[int, ...]` → `Unpack[tuple[int, ...]]` |
| empty declarations | `empty_declarations.rs` | `class A` → `class A: ...` |
| modifiers | `modifiers.rs` | keyword modifiers → decorators/annotations |
| overload | `overload.rs` | stacked signatures → `@overload` |
| none-coalescing | `coalesce.rs` | `a ?? b` → conditional expression |
| none-chaining | `none_chain.rs` | `a?.b` → guarded access |
| multiline dedent | `dedent_string.rs` | strips common indentation from triple-quoted strings |
| typed lambda | `typed_lambda.rs` | `lambda (a: int): ...` → `lambda a: ...` |

after all transforms run, their edits are merged, deduplicated, and applied to produce the final output

## source maps

`transpile_with_map` returns a `SourceMap` alongside the output string. the source map records how byte offsets in the generated Python correspond to byte offsets in the original `.by` source. this enables error messages and debuggers to point back to the correct line in the original file

## reverse transforms

basedpython also supports reverse transpilation — converting standard Python back into basedpython syntax. see the [reverse transforms](reverse-transforms.md) page for details

## preamble generation

transforms that require new imports (e.g. `from typing import TypeVar`) register them during the walk. after all edits are applied, the transpiler collects these imports and prepends them as a preamble to the output file
