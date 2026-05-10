# tuple type literals

a parenthesized tuple in an annotation position is rewritten to `tuple[...]`:

```by
point: (int, int)
record: (int, str, float)
nested: (int, (str, float))
maybe: (int, str) | None

def origin() -> (int, int):
    return (0, 0)
```

transpiles to:

```python
point: tuple[int, int]
record: tuple[int, str, float]
nested: tuple[int, tuple[str, float]]
maybe: tuple[int, str] | None

def origin() -> tuple[int, int]:
    return (0, 0)
```

## syntax

```text
tuple_type ::= "(" type ("," type)* [","] ")"
```

a parenthesized list of one or more types — trailing comma allowed. a
single-element form requires the trailing comma to disambiguate from a
parenthesized expression: `(int,)` is `tuple[int]`, while `(int)` is
just `int`

## scope

rewriting fires in syntactic type contexts only: parameter annotations,
return-type annotations, `AnnAssign` targets, type aliases, and subscript
slices that are themselves type contexts. value-context tuples (`x = (1, 2)`)
are untouched

## composition

the rule recurses into surrounding type forms — unions, callables,
generics, intersections — so any tuple type expression nested inside is
also rewritten:

```by
fns: list[(int) -> (str, int)]
# → list[Callable[[int], tuple[str, int]]]
```

## relation to anonymous named tuples

if any element in the parenthesized list uses `name : type` form, the
expression is recognised as an [anonymous named tuple](anonymous-named-tuple.md)
instead. tuple-type rewrite and anon-NT rewrite are exclusive: a tuple
either has all-positional fields (becomes `tuple[...]`) or contains at
least one named field (becomes a synthesized `NamedTuple` class)
