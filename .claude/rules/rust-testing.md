---
description: Rust testing discipline — TDD red-green-refactor, behavior-driven tests, bug-fix protocol, hypothesis-driven debugging, quality gates
paths: ['**/*.rs', '**/tests/**']
---

# Rust Testing

Runner is **cargo test** — unit tests in `#[cfg(test)]` modules alongside the code, integration tests in `tests/`.

```bash
cargo test                    # full suite
cargo test <name>             # single test
cargo test -- --nocapture     # with stdout
cargo test --doc              # documentation tests
```

## TDD: Write Tests First

- **Every new function/module starts with a failing test** — write it, watch it fail (red), implement the minimum to pass (green), then refactor. Don't skip the red step.
- Tests are executable specifications: when requirements are ambiguous, writing the test first forces clarity before implementation begins.
- Every new module gets a corresponding `#[cfg(test)]` module (or a `tests/<module>.rs` vertical-slice test). No exceptions.

## BDD: Test Behaviors, Not Implementation

- **Test WHAT the public interface does, not HOW** — never assert on private functions, internal state, or call counts.
- Refactoring must not break tests. If it does, the test was coupled to implementation details — fix the test, don't revert the refactor.
- Fewer meaningful behavior tests beat many trivial ones. Delete tests that don't verify a distinct user-facing behavior.
- Name tests after the behavior they verify: `expired_token_is_rejected`, not `test_check_token_method`.

## Bug Fixing Protocol

Failing-test-first (per the global CLAUDE.md protocol): write a failing test that reproduces the bug before touching code. The bug is resolved when the test passes. When delegating to subagents, frame the task as: "make this failing test pass."

## Hypothesis-Driven Debugging

For non-obvious bugs, don't shotgun-fix:

1. **Hypothesize** — state 2-3 testable root-cause theories before touching code.
1. **Test each hypothesis** — design a minimal check (log, assertion, or unit test) per theory; tag debug output with the hypothesis it tests (`[H1]`, `[H2]`).
1. **Confirm before fixing** — implement only after evidence confirms a hypothesis; if all fail, form new ones from what you learned.
1. **Clean up** — remove all debug instrumentation after the fix is verified. The final diff contains only the fix.

## Quality Gates

Before marking any task complete, pass these gates in order:

| Gate               | Check                                                           | Hard Rule                                 |
| ------------------ | --------------------------------------------------------------- | ----------------------------------------- |
| **Correctness**    | Does it do what was asked? Not more, not less.                  | Never ship untested behavior              |
| **Evidence**       | Did you verify with tests, logs, or manual confirmation?        | Evidence > instinct — always              |
| **Edge cases**     | Empty inputs, None, duplicates, unicode, large data considered? | Handle boundaries explicitly              |
| **Existing utils** | Checked for existing functions/modules before writing new ones? | Search before you create                  |
| **Staff review**   | Would a staff engineer approve this as-is?                      | If the honest answer is no — fix it first |

If any gate fails, fix it before claiming the task is done. Don't ship work you wouldn't defend in a code review.
