//! Reverse transforms: rewrite standard Python source into idiomatic basedpython.
//!
//! Each forward transform in `crate::transforms` polyfills a basedpython idiom
//! to standard Python; the corresponding reverse transform here detects that
//! polyfill output and rewrites it back. Together they enable round-tripping
//! for ecosystem testing — a Python file run through `reverse_transpile` and
//! then `transpile` should produce code with the same AST as the original.
//!
//! Reverse transforms are intentionally conservative: when in doubt, leave
//! the source unchanged. A false negative (missed rewrite) is a no-op; a
//! false positive can change semantics.

pub mod empty_class;
pub mod literal_types;
pub mod subscript;
