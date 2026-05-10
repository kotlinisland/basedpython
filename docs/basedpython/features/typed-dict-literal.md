# typed dict literals

a dict-shaped expression in an annotation position is a `TypedDict`:

```by
def get_user() -> {"name": str, "age": int}: ...
```

shape identity ignores key order — `{"a": int, "b": str}` and
`{"b": str, "a": int}` are the same type

`typing_extensions` is a runtime requirement of any module that contains a
typed-dict-literal annotation (the generated type uses PEP 728
`closed`/`extra_items` features that aren't yet in `typing.TypedDict`)

## closed by default, `**: T` for extra items

dict literal types are closed by default, so extra keys are rejected. a
`**: T` entry switches that to "extra keys allowed, must match T":

```by
a: {"key": int}              # closed — extra keys rejected
b: {"key": int, **: str}     # extra keys allowed, must be str
```

## nested shapes

nested typed-dict literals work transparently:

```by
addr: {"city": str, "zip": str}
user: {"name": str, "address": {"city": str, "zip": str}}
```

## scope

the rewrite fires only in annotation positions — function parameter and
return annotations, variable annotations, and nested type arguments. dict
expressions in value positions (real dicts) are never affected
