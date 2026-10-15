#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "Fill task bodies as raw markup and unescape the filed ones",
  tags: ("docs",),
  priority: framework("ice", confidence: 0.9, ease: 7.0, impact: 5.0),
  status: done(2026, 10, 15),
)

== Summary

The `summary` slot of `.mindtape/templates/task.typ` was `kind: "text"`, which `mt add` markup-escapes: every backtick in a filed body landed as a literal backslash-backtick, and the `<`, `_`, `#` and `[` inside the code span were escaped in turn, so `mmz --import <dir>` filed as an unreadable mess. 14 task files under `tasks/` carry the damage. Mindtape fixed the same template in its own repo (commits 98ce1e44 and 64dfa4f9). Changes: the slot becomes `raw`, so a body splices verbatim and a bare `#` or an unbalanced bracket fails `mt add` before anything is written; each filed task gets its code spans back, unescaping only what was meant as markup. Done when `mt check` passes on every task and no task holds an escaped backtick.
