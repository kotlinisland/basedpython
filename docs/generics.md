# generics

`ParamSpec` was a mistake, it doesn't actually unpack, so why does it have stars?

we propose that type parameter semantics should mirror value parameter semantics 1 to 1:
```bython
class A[Positional, /, PositionalOrNamed, *, Named]

class B[Positional, /, PositionalOrNamed, *Args, Named, **Kwargs]
```
`*Args` would behave the same as tvt, but `**Kwargs` would actually capture a typed dictionary of named arguments

```bython
A[
  int,  # Positional
  PositionalOrNamed=str,
  Named=int,
]

B[
  int,  # Positional
  str,  # PositionalOrNamed
  int,  # Args
  str,  # Args
  Named=int,
  foo=str,  # Kwargs
  bar=int,  # Kwargs
]
```

parameter specifications will be denoted via a new special form `Parameters`:

```bython
from typing import Parameters

def f[P: Parameters](fn: Callable[P, None]) -> Callable[P, int]: ...

# denoted via a special parameters syntax 
f[(int, str)]()

# more of this syntax
class A[P: Parameters = (int, *: str)]
A[(a: str, b: str)]
```

this also acts as a top type for `Callable` and any parameter specification:
```bython
from typing import Parameters

type CallableTopType = Callable[Parameters, object]
```
TODO: more top types like `Callable[Ts, None]`?

`Concatenate` will be replaced with a new unpack notation:
`***` means unpack positional and keyword items, a combination of `*` and `**`
```bython
def f[P: Parameters](fn: Callable[P, None] -> Callable[[int, ***P], None]:
```

`Callable` will expose it's type via "attributes as types" by forwarding to it's `Parameters` type parameter
```bython
class Callable[P: Parameters, R]:
    @type_check_only
    args: Parameters.args
    
    @type_check_only
    kwargs: Parameters.kwargs
    
    @type_check_only
    returns: R
    
class A[Fn: Callable[Parameters, object]]:
    def f(self, *args: tuple[Fn.args], **kwargs: dict[str, Fn.kwargs]) -> Fn.returns: ...
```
