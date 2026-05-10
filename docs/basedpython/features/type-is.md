# type narrowing predicates

basedpython spells [PEP 742][pep-742] `TypeIs[T]` as `name is T` in the return
annotation, naming the parameter being narrowed:

```by
def is_str(x) -> x is str:
    return isinstance(x, str)
```

transpiles to:

```python
from typing_extensions import TypeIs

def is_str(x) -> TypeIs[str]:
    return isinstance(x, str)
```

## semantics

the runtime semantics are exactly PEP 742 — the function asserts that its
argument has type `T` when it returns `True`, and ty narrows accordingly at
call sites. the parameter name is lost in lowering (`TypeIs` doesn't carry it)
but is preserved in the source for readers

## scope

the rewrite fires only when the return annotation is a single `name is T`
comparison where `name` is a bare identifier. this disambiguates from
identity checks elsewhere in the function:

- in the return annotation: `x is str` → `TypeIs[str]`
- anywhere else: `x is y` follows the [identity-swap rules](identity-swap.md)
    and lowers to `isinstance(x, y)`

chained comparisons (`a is int is str`) and non-identifier left operands are
ignored

[pep-742]: https://peps.python.org/pep-0742/
