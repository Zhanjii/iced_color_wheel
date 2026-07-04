<!-- TEMPLATE:RUST START -->
# Rust Conventions (digest)

- GUI stack is **iced 0.14 daemon** exclusively — never Tauri, egui, eframe, or web frontends. Themed dialogs via `iced_message_box`, never `rfd`.
- Async in `update()` returns `Task::perform(...)` — `Command` was renamed `Task` in iced 0.13; the old API does not exist in 0.14.
- Release builds set `windows_subsystem = "windows"` (console hidden) — use `tracing` macros for all operational output; `println!`/`eprintln!` output silently vanishes.
- TDD-first: every new function/module starts with a failing test (red → green → refactor); name tests after behaviors (`expired_token_is_rejected`).
- Any task with 3+ steps: stop and outline steps + assumptions before writing code. Non-negotiable.
- Read `SECURITY.md` in the project root (if present) before flagging security issues — it documents intentional tradeoffs.
- Org style guide is authoritative: Read `claude-setup/docs/style-guide-rust.md` + `org-conventions.md` before non-trivial work; announce org-enforced vs fallback mode.
- De-slop pass on the diff before every commit; final sweep: `cargo fmt --check && cargo clippy -- -D warnings && cargo test`.
- Cloning to silence the borrow checker is a smell — restructure the data flow. Prefer `&str` params over `&String`, `impl Trait` over `dyn Trait`.
- Rust cardinal rules (no `.unwrap()`/`.expect()`, `.context()` on every `?`, `DateTime<Utc>`) and PostgreSQL conventions: global CLAUDE.md.

## Detailed conventions

`.claude/rules/*.md` lazy-load by file path (`paths:` frontmatter) — no need to pre-read:

- `rust-conventions.md` — org precedence, principles, agent behavior, gotchas, TODO format (any `.rs` / `Cargo.toml`)
- `rust-iced-gui.md` — daemon patterns, dependency block, iced gotchas, theme/style modules (`src/ui/`, `app.rs`, `main.rs`)
- `rust-testing.md` — TDD/BDD, bug-fix protocol, hypothesis debugging, quality gates (any `.rs`, `tests/`)

Read on demand when deeper detail is needed:

- Full template: `H:/BOOK.IO/02_ELEMENTS/10_SCRIPTS/00_LLM_language_specific_instructions/RUST/CLAUDE.md`
- Org style guide: `H:/BOOK.IO/02_ELEMENTS/10_SCRIPTS/claude-setup/docs/style-guide-rust.md`

Machine-wide conventions (git, PR process, testing requirements, memory, Obsidian vault) come from the global CLAUDE.md — not duplicated here.
<!-- TEMPLATE:RUST END -->

<!-- PROJECT-SPECIFIC START -->
<!--
  ╔══════════════════════════════════════════════════════════════╗
  ║  PROJECT-SPECIFIC INSTRUCTIONS                              ║
  ║                                                             ║
  ║  Add your project's unique context below. This section      ║
  ║  is separate from the template above so template updates    ║
  ║  don't overwrite your project-specific content.             ║
  ║                                                             ║
  ║  Suggested sections:                                        ║
  ║  ## Project Overview          — what this project does      ║
  ║  ## Architecture              — key design decisions        ║
  ║  ## Key Files & Entry Points  — where to start reading      ║
  ║  ## Environment & Secrets     — env vars, API keys needed   ║
  ║  ## Known Issues              — gotchas and workarounds     ║
  ╚══════════════════════════════════════════════════════════════╝
-->
<!-- PROJECT-SPECIFIC END -->
