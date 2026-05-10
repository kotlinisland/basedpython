# lazy imports

every `import` and `from import` statement in a `.by` file is automatically
marked lazy. the transpiler prepends the `lazy` keyword
([PEP 810](https://peps.python.org/pep-0810/), Python 3.15+) so the
runtime defers module loading until first use

```by
import os

print(os)
```

transpiles to:

```python
lazy import os

print(os)
```

Python 3.15's runtime registers `os` in `sys.modules` immediately but
defers executing its body until the first attribute access on the module
object. accessing `os.sep` (or `print(os)`, which calls `__repr__`) is
what actually loads the module

## supported forms

| basedpython                   | Python output                      |
| ----------------------------- | ---------------------------------- |
| `import os`                   | `lazy import os`                   |
| `import os as o`              | `lazy import os as o`              |
| `import os.path as p`         | `lazy import os.path as p`         |
| `from os import path`         | `lazy from os import path`         |
| `from os import path as p`    | `lazy from os import path as p`    |
| `from os import path, getcwd` | `lazy from os import path, getcwd` |
| `from .pkg import x`          | `lazy from .pkg import x`          |

`import a.b` without an alias stays eager (write `import a.b as ab` to opt
in). `from __future__ import …` and `from x import *` are always eager

## target version

on python 3.15 and later, the PEP 810 `lazy` keyword is used directly.
on older runtimes, a runtime polyfill is emitted. `from __future__` and
`from x import *` are always left eager.

set the target with `--min-version 3.15` on `by transpile`/`by build`/`by run`
(`by check` uses `--python-version` for the same concept)
