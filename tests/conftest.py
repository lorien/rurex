"""Root conftest: refuse to run with a replaced stdlib ``re``.

This runs at conftest import time, before pytest's collection hooks can
install the ``tests/cpython`` shim, so it is only a fast fail for a
shadowed interpreter (a plugin, a future shared bootstrap). The
authoritative checks live next to each oracle binding in
``tests/rustregex/parity.py`` and ``tests/divergences/red_live.py``,
because the shim is installed *after* this file has been imported.
"""

import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).parent))

from re_guard import require_stdlib_re  # noqa: E402

require_stdlib_re()
