---
description: Core Rust conventions — org style-guide precedence, core principles, agent behavior, common gotchas, TODO format, de-slop pass
paths: ['**/*.rs', '**/Cargo.toml']
---

# Rust Conventions

## Authoritative Org Conventions (Read First)

The `claude-setup` repo is the **authoritative source** for Rust conventions; these rules layer capabilities on top — they do not replace it.

- `H:/BOOK.IO/02_ELEMENTS/10_SCRIPTS/claude-setup/docs/style-guide-rust.md` — cardinal rules, Rocket patterns, SQLx patterns, 3-struct data model, migration naming
- `H:/BOOK.IO/02_ELEMENTS/10_SCRIPTS/claude-setup/docs/org-conventions.md` — testing, logging, git, PR process, challenge workflow

Procedure: attempt to Read both before writing or reviewing Rust code. If present, they win any conflict with local guidance. If either is missing, emit:

> ⚠️ Org style guide not loaded at `<path>`. Operating in fallback mode against local rules only. Org-specific rules (e.g., `.context()` on `?`, 3-struct pattern, migration naming) will NOT be enforced.

Never silently skip this check — state which mode is active (org-enforced vs fallback) at the start of non-trivial work.

## Project Commands

```bash
cargo check                                   # fast compile check, no codegen
cargo test <name> -- --nocapture              # single test with stdout
cargo clippy -- -D warnings                   # lint, warnings as errors
cargo fmt --check && cargo clippy -- -D warnings && cargo test   # full validation
```

## Core Principles

1. **Safe** — avoid `unsafe` unless absolutely necessary; document safety invariants when used
1. **Expressive** — encode invariants in types; prefer strong typing over stringly-typed data
1. **Minimal** — absolute minimum code needed; avoid unnecessary allocations and cloning
1. **Self-documenting** — doc comments (`///`) on public APIs; type signatures that convey intent
1. **Performant** — profile before optimizing; async for I/O-bound operations

**Ownership rule of thumb**: default to borrowing (`&T`); use owned types when the function stores or returns the data; `Cow<'_, T>` when either case may apply.

## Agent Behavior

1. **Surface assumptions** — state assumptions explicitly before non-trivial work; never silently fill in ambiguous requirements.
1. **Stop when confused** — on inconsistent or conflicting requirements: name the confusion, present the tradeoff, wait for resolution. Don't guess and hope.
1. **Push back when warranted** — if the approach has clear problems, point them out and propose an alternative; accept the override.
1. **Dead code hygiene** — after refactoring, list newly unreachable code explicitly and ask before removing.
1. **Root cause discipline** — is the fix root cause or symptom? If symptom, research deeper before implementing.
1. **Demand elegance** — before a hacky fix, ask *"knowing everything I know now, what's the elegant solution?"* If a clean path exists, take it — hacks compound.

## Before Coding

For non-trivial work, weigh implementation axes before writing: owned vs borrowed data, sync vs async, trait objects vs generics vs enums. Choose the simplest approach — concrete types before generics, composition over inheritance. Mark deferred work explicitly: `// TODO(scope): …` or `todo!()` / `unimplemented!()`.

## De-Slop Pass

A dedicated cleanup pass after code compiles and tests pass, before `git add`. Run against the diff (`git diff`), not the whole file — this catches the slop clippy can't:

- **Naming drift** — `user_id` *and* `userId` in one diff? Pick `snake_case` (RFC 430); don't mix `fetch_`/`get_` verbs for the same operation.
- **Speculative abstractions** — trait with one impl, `Box<dyn …>` where a generic fits, once-used wrapper struct? Inline it. One adapter is a hypothetical seam; two is a real one.
- **Stale doc comments** — do `///`, `# Errors`, `# Panics` still match the function? Fix or delete.
- **DIAG_DEBUG residue** — strip leftover `dbg!()`, `eprintln!()`, and diagnostic `tracing::debug!` lines; production uses `tracing` at appropriate levels.
- **Half-finished branches** — empty `if let` arms, unjustified `unreachable!()`, forgotten `todo!()`: implement or remove.
- **Orphaned `use` statements** — let `cargo clippy --fix` sweep them.
- **Error context** — every new `?` carries `.context("…")`; every new public `Result` documents its error type. No bare `?` chains.
- **Test coverage gap** — each new code path has a `#[test]` (or `tests/` vertical-slice test), or an explicit `// TODO(test): cover <path>`.

Finish with `cargo clippy --all-targets --all-features -- -D warnings && cargo fmt` — a mechanical sweep, not a substitute for the judgment items above.

## After Coding

Update docs: Glob `docs/**/*.md` for the area you touched; update matches, suggest a well-named doc for new features, and verify signatures/examples/config in docs match the implementation. Never claim "Code Complete" while TODO/FIXME/INCOMPLETE comments remain in scope.

## Inline TODO Format

```rust
// TODO(scope): description       — general work to be done
// FIXME(bug): …                  — known bug or issue to fix
// INCOMPLETE(api): …             — partial implementation
// DEFERRED(perf): …              — intentionally postponed
// HACK: …                        — temporary workaround
```

## Common Gotchas — Where Claude Gets Rust Wrong

### Ownership & Borrowing

- **Cloning to silence the borrow checker** — clone is a code smell; restructure the data flow first.
- **`&String` in params** — always `fn foo(s: &str)`; it accepts both `String` and slices.
- **Moving out of `&self`** — clone only if needed, or restructure to return a reference.
- **Unnecessary explicit lifetimes** — one reference in, one out: elision handles it. Annotate only when the compiler asks.

### Type System

- **`dyn Trait` by default** — prefer `impl Trait` (monomorphized, zero-cost) for function params/returns.
- **`.unwrap()` in production** — use `?`, `.map_err()`, or `match`; `unwrap()` is for tests only.
- **Returning `String`** where `&str` / `Cow<str>` would avoid the allocation.
- **Turbofish** — remember `::<Type>` when inference fails, especially with `collect()`.

### Async

- **Blocking in async context** — use `tokio::fs` and `tokio::time::sleep`, never `std::fs::read()` / `thread::sleep()` in async fns.
- **Missing `Send` bounds** — don't hold non-Send types across `.await` points; tokio will reject the future.
- **Missing `#[tokio::main]`** on async main — the errors are confusing without it.

## Windows Notes

- Use `std::path` types — `PathBuf::from("C:/…")` works with forward slashes on Windows; never build paths by string concatenation.
- `git config --global core.autocrlf true`; rustfmt handles line endings automatically.

## Reference Documentation

- The Rust Book: https://doc.rust-lang.org/book/ · Std: https://doc.rust-lang.org/std/
- Reference: https://doc.rust-lang.org/reference/ · By Example: https://doc.rust-lang.org/rust-by-example/
- tokio: https://docs.rs/tokio/latest/tokio/ · serde: https://serde.rs/ · crates.io for dependency versions

(iced references live in `rust-iced-gui.md`.)
