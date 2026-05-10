# default argument re-evaluation

a long-standing python footgun is that mutable default values (`[]`, `{}`,
`set()`, etc.) are evaluated once at function-definition time and shared
across calls. basedpython removes the footgun: every non-scalar default is
re-evaluated per call, so each call gets a fresh value:

```by
def append_one(items=[]):
    items.append(1)
    return items
```

`append_one()` returns `[1]` every time, never the accumulating list.

scalar literals — numbers, bools, `None`, strings, `...` — stay as plain
python defaults (they're immutable, cheap, and carry no hidden state).
everything else is rewritten so the default expression runs at call time.

a useful side effect: `def g(a, b=a + 1)` becomes valid late-bound default
syntax, with `b` computed fresh from the actual `a` at each call
