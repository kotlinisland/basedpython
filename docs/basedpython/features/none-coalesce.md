# none-coalesce operator

`a ?? b` evaluates to `a` when `a is not None`, otherwise to `b`:

```by
name = user.display_name ?? "anonymous"
```

transpiles to:

```python
name = user.display_name if user.display_name is not None else "anonymous"
```

## semantics

`??` tests `is not None` (identity) — not falsiness. an empty string, zero,
or an empty list is *not* coalesced. only `None` triggers the fallback

## precedence

`??` has the same precedence as a conditional expression — it binds looser
than `or` / `and` / arithmetic and tighter than assignment. parenthesize when
mixing with boolean operators

## interaction with `?.`

see [optional chaining](optional-chaining.md). `??` and `?.` compose; complex
left operands are evaluated at most once even when chained
