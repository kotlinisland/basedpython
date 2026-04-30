# visibility modifiers

basedpython provides `export` (alias `public`) and `private` keywords to control module-level symbol visibility

## transformation rules

| basedpython | Python output |
|---|---|
| `export def api():` | `def api():` + added to `__all__` |
| `public def api():` | `def api():` + added to `__all__` |
| `private def helper():` | `def _helper():` |
| `def untouched():` | `def untouched():` *(unchanged)* |

## example

```python
# basedpython
export def api(): ...
private def helper(): ...
def untouched(): ...
```
```python
# generated Python
def api(): ...
def _helper(): ...
def untouched(): ...
__all__ = ["api"]
```

## behavior

- `export`/`public` strips the modifier keyword and adds the symbol's name to an auto-generated `__all__` list at the end of the module
- `private` strips the modifier keyword and renames the declaration with a leading underscore — the conventional Python "internal" marker
- unmarked declarations are left unchanged and are not included in `__all__`
- both modifiers apply to `def` and `class` at module scope
- inside a class body the modifier keyword is stripped without renaming or affecting `__all__`
