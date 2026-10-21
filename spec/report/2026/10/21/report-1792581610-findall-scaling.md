## Report on task: Python result objects on free-threaded Python

Owner instruction: "do it". The task was measured to the end. Both
implementation attempts were reverted (mixed or unmeasurable), and what
remains is an API decision, recorded as a re-scoped plan task. No record
is archived: the task stays in `plan.md` under a sharper title.

### Done

- **Measured the task's premise against stdlib `re`**, which is the only
  fair reference: on 200k Telegram posts, per-post wall cost of one
  worker at 1 versus 4 free-threaded workers, one case per process and
  threads created once:

  | engine | op | 1 worker | 4 workers |
  |---|---|---:|---:|
  | rurex | `findall` | 167 ns | 302 ns (1.8× worse) |
  | `re` | `findall` | 208 ns | **195 ns (scales)** |
  | rurex | `finditer` count | 250 ns | 679 ns (2.7×) |
  | `re` | `finditer` count | 317 ns | 1075 ns (3.4×) |

  The per-`Match` path is *better* than `re`'s at both worker counts, so
  "result objects" was the wrong diagnosis for `finditer`; the one real
  regression is `findall`, and it is the Telegram bytes row.
- **Tried the two binding-side levers that exist for it.**
  - Thread-local reuse of the flat span buffer (`Vec` per `findall` call
    replaced by a thread-local buffer with a nested-call fallback): no
    measurable change at either worker count (290–308 ns before,
    288–301 after, three alternating rounds).
  - Borrowing the haystack instead of cloning its `Bound` per call (one
    foreign-object refcount pair per call, which `re` never pays because
    a C method's argument comes borrowed from the vectorcall array):
    `is_match` 94 → 88 ns at one worker, 99 → 93 at four; `findall`
    166 → 160 and 296 → 293; **`finditer` 678 → 721 at four workers**,
    consistent over five alternating rounds while executing 1% fewer
    instructions. A wash with a regression in it, so it was reverted;
    the finding is recorded in the plan task in case a future change
    wants it split by path.
- Both changes passed the full suite and parity checks before being
  reverted; the tree is back at `617a476`.
- Re-scoped the plan record to the decision the measurements lead to,
  and added the corresponding paragraph to `architecture.md`.

### Results

`findall` at four workers spends 48% of self time in CPython's shared
reference-count paths (`_Py_IncRef` 21.9%, `Py_DECREF` 15.3%,
`_Py_DecRefShared` 11.3%) against 16% at one, with rurex's own frames
around 12% of the profile (engine ~7%, `findall_from` + closure ~5%).
The engine itself scales (ADR-0019: `is_match` 1.04× at four workers),
so the loss is per-value Python object traffic — which `findall` cannot
avoid while it returns a list of values.

Cross-checks used: `perf stat -e task-clock,instructions` (the finditer
regression executes 1% *fewer* instructions while taking 5% longer,
i.e. it is stall-bound, not work-bound), and five alternating rounds per
wheel for every wall-clock claim under 10%.

### Spec/ADR amendments

- [acted] `spec/docs/plan.md`: the record is re-scoped to "`findall`
  does not scale on free-threaded Python (needs a decision)" with the
  `re` comparison, the two rejected levers and the measurement recipe.
- [acted] `spec/docs/architecture.md` "Matching path" states which
  workload does not scale, that the engine does, and that the remaining
  lever is an API one.
- [acted] No new ADR: the two attempts were reverted, so there is no
  decision to record beyond the plan record; ADR-0019 already covers the
  engine side.

### Future-task notes

- [open] The decision the record asks for: a counting entry point that
  returns an integer instead of a list of values (`Pattern.count`, say).
  It would fix the row outright, but `re` has no equivalent and the
  `re`-compatible spelling is `len(re.findall(...))`, so it is an API
  addition the owner has to want.
- [open] If the haystack borrow is revisited, split it: it helped
  `is_match` (5–7%) and `findall` (2–3%) and hurt `finditer` (5% at four
  workers). The iterator path holds its own `Py<PyAny>` string, so it
  does not need the borrowed view at all.
- [open] `Match` is ~160 bytes against CPython's ~104; its inline span
  buffer (ADR-0016) was measured neutral single threaded and is
  untouched here. A smaller `Match` is the only remaining object-side
  lever, and the `re` comparison says it is not what costs.

### Tooling/process

- The `re` comparison is what turned this task around: without it the
  numbers looked like "rurex result objects scale badly", and with it,
  `finditer` is ahead of `re` and only `findall` is behind. Any future
  free-threaded claim should carry the `re` column next to it.
- Instruction counts settle "is this a real work difference or a stall
  difference" in one command; they disagreed with wall clock here and
  explained the reverted change rather than contradicting it.
- The throwaway probe extension built for the previous session
  (`/tmp/probe-ext`, pyo3 + regex, maturin) is what made the frozen /
  mutable / argument-shape comparisons cheap; the per-process probe
  scripts (`/tmp/tgall.py`, `/tmp/tgiter.py`, `/tmp/tgops2.py`) are the
  reusable instrument and the recipe is in the plan record.
