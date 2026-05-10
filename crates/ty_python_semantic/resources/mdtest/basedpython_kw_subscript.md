# basedpython: keyword arguments in subscripts

`T[name=int]` binds the type parameter named `name` on `T` to `int`. for multi-typevar generics,
mixing positional and keyword arguments is permitted; for single-typevar generics, the keyword name
is dropped.

```toml
[environment]
python-version = "3.12"
```

## explicit binding for two-typevar class

```by
class M[K, V]: ...

def f(x: M[K=int, V=str]) -> None:
    reveal_type(x)  # revealed: M[int, str]
```

## single-typevar drops the keyword

```by
class B[T]: ...

def f(x: B[T=int]) -> None:
    reveal_type(x)  # revealed: B[int]
```
