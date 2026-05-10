//! Pre-source text-edit that strips use-site variance markers (`out T`,
//! `in T`, `in out T`) from the source string before the main lowering
//! pipeline begins.
//!
//! Unlike the other passes in `ast_pass`, this one does NOT mutate the AST
//! and re-render through [`Generator`]; it scans the AST for variance
//! markers, gathers their source ranges, and deletes them from the source
//! string directly. The result is a basedpython source file with no
//! variance keywords — downstream transforms (callable arrow lowering,
//! intersection lowering) can then copy operand source verbatim without
//! capturing variance keywords that would later leak
//!
//! Pure-deletion edits like this don't have an overlap problem with other
//! transforms' edits, because we apply them to the source upstream of the
//! whole text-edit pipeline. They also preserve every other byte of
//! formatting — the only change to the source is the variance keyword
//! (and one trailing space) being removed at each marker site.

use ruff_python_ast::PySourceType;
use ruff_python_ast::helpers::use_site_variance_marker;
use ruff_python_ast::visitor::{Visitor, walk_expr, walk_stmt};
use ruff_python_ast::{Expr, Stmt};
use ruff_python_parser::parse_unchecked_source;
use ruff_text_size::Ranged;

/// Strip every use-site variance marker (`out T`, `in T`, `in out T`)
/// from `source` and return the cleaned source. If parsing fails or no
/// markers are present, returns `source` unchanged.
pub(crate) fn strip(source: &str) -> std::borrow::Cow<'_, str> {
    let parsed = parse_unchecked_source(source, PySourceType::BasedPython);
    if !parsed.errors().is_empty() {
        return std::borrow::Cow::Borrowed(source);
    }
    let module = parsed.into_syntax();

    let mut collector = MarkerCollector { ranges: Vec::new() };
    for stmt in &module.body {
        collector.visit_stmt(stmt);
    }
    if collector.ranges.is_empty() {
        return std::borrow::Cow::Borrowed(source);
    }
    let mut ranges = collector.ranges;
    // sort descending by start so position-based edits stay valid
    ranges.sort_by_key(|r| std::cmp::Reverse(r.0));
    let mut out = source.to_owned();
    for (start, end) in ranges {
        out.replace_range(start..end, "");
    }
    std::borrow::Cow::Owned(out)
}

struct MarkerCollector {
    ranges: Vec<(usize, usize)>,
}

impl<'ast> Visitor<'ast> for MarkerCollector {
    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &'ast Expr) {
        if let Some((_, inner)) = use_site_variance_marker(expr) {
            // delete the variance keyword bytes between the outer marker
            // range and the inner expression's start
            let start = usize::from(expr.range().start());
            let inner_start = usize::from(inner.range().start());
            if start < inner_start {
                self.ranges.push((start, inner_start));
            }
        }
        walk_expr(self, expr);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::python_passthrough::unchanged;
    use crate::{Config, transpile};
    use indoc::indoc;

    fn check(input: &str, expected: &str) {
        assert_eq!(transpile(input, &Config::test_default()).unwrap(), expected);
    }

    #[test]
    fn def_site_covariant_stripped() {
        check(
            "class Box[out T]: ...\n",
            indoc! {"
                from typing import TypeVar, Generic
                _T = TypeVar(\"_T\", covariant=True)
                class Box(Generic[_T]): ...
            "},
        );
    }

    #[test]
    fn def_site_contravariant_stripped() {
        check(
            "class Sink[in T]: ...\n",
            indoc! {"
                from typing import TypeVar, Generic
                _T = TypeVar(\"_T\", contravariant=True)
                class Sink(Generic[_T]): ...
            "},
        );
    }

    #[test]
    fn def_site_bivariant_stripped() {
        check(
            "class Box[in out T]: ...\n",
            indoc! {"
                from typing import TypeVar, Generic
                _T = TypeVar(\"_T\")
                class Box(Generic[_T]): ...
            "},
        );
    }

    #[test]
    fn use_site_out_strips_keyword() {
        check(
            "def f(data: list[out int]) -> int: ...\n",
            "def f(data: list[int]) -> int: ...\n",
        );
    }

    #[test]
    fn use_site_in_strips_keyword() {
        check(
            "def f(data: list[in int]) -> None: ...\n",
            "def f(data: list[int]) -> None: ...\n",
        );
    }

    #[test]
    fn use_site_in_out_strips_keyword() {
        check(
            "def f(data: list[in out int]) -> int: ...\n",
            "def f(data: list[int]) -> int: ...\n",
        );
    }

    #[test]
    fn use_site_does_not_fire_on_plain_subscript() {
        unchanged("a: list[int]\n");
    }

    #[test]
    fn use_site_does_not_fire_on_bare_out_identifier() {
        unchanged("x = out\n");
    }

    #[test]
    fn use_site_does_not_fire_on_arithmetic_continuation() {
        unchanged("y = a[out + 1]\n");
    }

    #[test]
    fn use_site_complex_inner_strips_keyword() {
        check("x: list[out int | str]\n", "x: list[int | str]\n");
    }

    #[test]
    fn use_site_multi_arg_strips_each_marked_element() {
        check(
            "def f(data: dict[str, out int]) -> int: ...\n",
            "def f(data: dict[str, int]) -> int: ...\n",
        );
    }

    #[test]
    fn use_site_nested_inside_other_subscript() {
        check("x: tuple[list[out int]]\n", "x: tuple[list[int]]\n");
    }

    #[test]
    fn strips_use_site_variance() {
        let out = strip("def f(x: list[out int]) -> None: ...\n");
        assert_eq!(out, "def f(x: list[int]) -> None: ...\n");
    }

    #[test]
    fn strips_inside_callable_arrow() {
        let out = strip("fn: (list[out int]) -> None\n");
        assert_eq!(out, "fn: (list[int]) -> None\n");
    }

    #[test]
    fn strips_inside_intersection() {
        let out = strip("def h(x: list[out int] & list[out str]) -> None: pass\n");
        assert_eq!(out, "def h(x: list[int] & list[str]) -> None: pass\n");
    }

    #[test]
    fn no_markers_borrows_input() {
        let src = "def f(x: list[int]) -> None: ...\n";
        let out = strip(src);
        assert!(matches!(out, std::borrow::Cow::Borrowed(_)));
        assert_eq!(out, src);
    }
}
