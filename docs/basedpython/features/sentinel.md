# sentinel

basedpython adds a `sentinel` statement for declaring named sentinel objects:

```by
sentinel MISSING
```

transpiles to:

```python
from typing_extensions import Sentinel

MISSING = Sentinel("MISSING")
```

## syntax

`sentinel NAME` declares a module-level sentinel. the form is a soft keyword
followed by a single identifier and a newline. the identifier becomes the
sentinel name (used both as the python variable and as the string passed to
`sentinel(...)`)

```by
sentinel MISSING
sentinel UNSET
```

## scope

`sentinel` is a soft keyword: in any other position (`sentinel = 5`,
`sentinel(...)`) it parses as a regular identifier

## polyfill

[PEP 661](https://peps.python.org/pep-0661/) proposes a builtin `sentinel`
function. until python ships it, `typing_extensions.Sentinel` is used
