<!-- TEMPLATE:RUST START -->
# Rust Coding Guidelines for Claude CLI (Iced Daemon Stack)

## Project Commands

```bash
# Development
cargo build              # Build the project
cargo build --release    # Build for release
cargo run                # Run the project
cargo run --release      # Run release build

# Testing
cargo test               # Run all tests
cargo test -- --nocapture  # Run tests with stdout
cargo test <name>        # Run specific test
cargo test --doc         # Run documentation tests

# Code Quality
cargo fmt                # Format code with rustfmt
cargo clippy             # Run Clippy linter
cargo clippy -- -D warnings  # Clippy with warnings as errors
cargo check              # Fast compile check without codegen

# Documentation
cargo doc                # Generate documentation
cargo doc --open         # Generate and open docs

# Full Validation
cargo fmt --check && cargo clippy -- -D warnings && cargo test
```

## Core Principles

1. **Safe** - Avoid `unsafe` unless absolutely necessary; document safety invariants when used
2. **Expressive** - Encode invariants in types; prefer strong typing over stringly-typed data
3. **Minimal** - Absolute minimum code needed; avoid unnecessary allocations and cloning
4. **Self-documenting** - Doc comments (`///`) on public APIs; type signatures that convey intent
5. **Performant** - Profile before optimizing; consider async for I/O-bound operations

**Ownership rule of thumb**: Default to borrowing (`&T`); use owned types when the function needs to store or return the data. Use `Cow<'_, T>` when either case may apply.

## GUI Stack: Iced Daemon

All desktop applications use **iced** (v0.14) with the **daemon** entry point for multi-window support.

### Key Patterns
- **Entry point**: `iced::daemon("App Name", App::update, App::view).run_with(App::new)`
- **Multi-window**: `HashMap<window::Id, WindowKind>` registry, dispatch `view()` by window ID
- **Popups**: `window::open(window::Settings { ... })` for settings, about, color picker, etc.
- **Close handling**: `window::close_events().map(Message::WindowClosed)` subscription
- **Daemon mode**: App does NOT exit when last window closes (pairs with system tray)
- **Color picker**: Use `iced_color_wheel` crate (v0.1) for HSV color wheel widget

### Why Daemon (Not Application)
- `iced::application` exits when its single window closes
- `iced::daemon` keeps running — required for tray-resident apps and popup workflows
- Each window gets independent state and view dispatch

### Dependencies
```toml
iced = { version = "0.14", features = ["multi-window", "canvas", "tokio"] }
iced_color_wheel = "0.1"
rfd = "0.15"       # Native file/message dialogs
tray-icon = "0.19"  # System tray (optional)
```

### IMPORTANT: Never Use Tauri or egui
This stack uses iced exclusively for GUI. Do not introduce Tauri, egui, eframe, or web frontend dependencies. All UI is pure Rust via iced widgets and canvas.

## Agent Behavior

1. **Surface assumptions** - Before implementing anything non-trivial, state your assumptions explicitly. Never silently fill in ambiguous requirements.
2. **Stop when confused** - If you encounter inconsistencies or conflicting requirements, stop. Name the confusion, present the tradeoff, and wait for resolution. Don't guess and hope.
3. **Push back when warranted** - You are not a yes-machine. If the human's approach has clear problems, point out the issue, explain the downside, and propose an alternative. Accept their decision if they override.
4. **Dead code hygiene** - After refactoring, identify code that is now unreachable. List it explicitly and ask before removing. Don't leave corpses, don't delete without asking.
5. **Root cause discipline** - When debugging, always ask whether the fix addresses the root cause or just a symptom. If it's a symptom, research deeper before implementing.

## CLI Tool Rules

- **Read before editing** - Always use Read to understand existing code before any modifications.
- **Use Edit/MultiEdit for changes** - Prefer MultiEdit when making multiple changes to the same file. Use Write only for new files.
- **Absolute paths only** - Never use relative paths in any tool call. Use Glob to find files by pattern rather than hardcoding paths.
- **Use Grep/Glob, not bash** - Never use bash `grep`, `find`, or `cat`. Use the Grep, Glob, and Read tools instead.
- **Use TodoWrite for complex tasks** - Track multi-step work with TodoWrite (pending -> in_progress -> completed).
- **Scan before declaring done** - Never claim "Code Complete" while TODO/FIXME/INCOMPLETE comments remain.

## Before Coding Process

For non-trivial work, consider alternative approaches before implementing:

1. **Identify core requirements** - What problem, what types/lifetimes, what error conditions, what traits?
2. **Consider implementation axes**:
   - Owned vs borrowed data
   - Sync vs async execution
   - Trait objects vs generics vs enums
3. **Choose simplest approach** - Concrete types before generics; composition over inheritance
4. **Verify**: Compiles without warnings? Error cases handled? Ownership clear? Idiomatic?
5. **Mark deferred work** - `// TODO(scope): description` or `todo!()` / `unimplemented!()` macros

## After Coding Process

When you complete implementation work, **always update relevant documentation**:

1. **Find corresponding docs** - Search with Glob (`docs/**/*.md`) or Grep for feature/module names
2. **Update what you find** - If a doc exists for your area, update it to match your changes
3. **Suggest creating if missing** - For new features, suggest a well-named doc (e.g., `docs/user-authentication.md`)
4. **Verify doc accuracy** - API signatures, code examples, and config options match the implementation

## Inline TODO Strategy

Use inline TODOs for persistent work items that live in the codebase.

### TODO Format Standards

```rust
// TODO(scope): Clear description of what needs to be done
// TODO(auth): Implement password reset token expiration check
// FIXME(bug): Handle edge case when user_id is None
// INCOMPLETE(api): Add rate limiting to this endpoint
// DEFERRED(perf): Consider caching user lookups after profiling
```

### TODO Categories

| Prefix | Use Case |
|--------|----------|
| `TODO` | General work to be done |
| `FIXME` | Known bug or issue to fix |
| `INCOMPLETE` | Partial implementation |
| `DEFERRED` | Intentionally postponed |
| `HACK` | Temporary workaround |

### Discovering TODOs

```bash
# Windows PowerShell
Select-String -Path "src\**\*.rs" -Pattern "TODO|FIXME|INCOMPLETE|DEFERRED"

# Git Bash / Linux / Mac
grep -rn "TODO\|FIXME\|INCOMPLETE\|DEFERRED" src/

# Or use the project's Python script
python scripts/list_todos.py
```

## Windows-Specific Considerations

Since you're on Windows:

1. **Path Handling**
   ```rust
   use std::path::{Path, PathBuf};

   // Use Path for cross-platform compatibility
   let path = PathBuf::from(r"C:\Users\Username\project");

   // Or use forward slashes (works on Windows too)
   let path = PathBuf::from("C:/Users/Username/project");
   ```

2. **Line Endings**
   - Configure git: `git config --global core.autocrlf true`
   - Rustfmt handles line endings automatically

3. **Console Output**
   - Use `ansi_term` or `colored` for cross-platform colored output
   - Enable ANSI support in Windows Terminal

## Multi-Instance Workflow

When running multiple Claude Code sessions in parallel:

- Number terminal tabs (1-5) for easy identification of parallel sessions
- System notification hooks (already configured via `claude_alert.py`) will alert you when input is needed
- Each instance should work on independent files/features to avoid merge conflicts
- Use `git worktree` for parallel work on the same repo without branch switching:
  ```bash
  git worktree add ../project-feature-branch feature-branch
  ```
- Keep a dedicated terminal for git operations (merging, rebasing) separate from Claude sessions

## Session Handoff

Claude Code supports handing off sessions between devices and interfaces:

- Use `&` to hand off a session from local CLI to claude.ai/code (and vice versa)
- Use `--teleport` flag to start a session that can move between devices
- Sessions can be started from phone via the Claude iOS app and picked up on desktop
- Useful for starting exploratory work on mobile and continuing implementation on desktop

## Reference Documentation

When debugging issues or answering questions, consult these sources:

### Rust Language
- **The Rust Book**: https://doc.rust-lang.org/book/
- **Std Library**: https://doc.rust-lang.org/std/
- **Rust Reference**: https://doc.rust-lang.org/reference/
- **Rust by Example**: https://doc.rust-lang.org/rust-by-example/

### Iced GUI Framework
- **iced repo + examples**: https://github.com/iced-rs/iced
- **iced docs.rs (latest)**: https://docs.rs/iced/latest/iced/
- **iced daemon entry point**: https://docs.rs/iced/latest/iced/fn.daemon.html
- **iced::window module** (open, close, settings): https://docs.rs/iced/latest/iced/window/index.html
- **iced::widget module** (button, text, column, etc.): https://docs.rs/iced/latest/iced/widget/index.html
- **iced::widget::canvas** (custom rendering): https://docs.rs/iced/latest/iced/widget/canvas/index.html
- **iced::Subscription** (events, timers, channels): https://docs.rs/iced/latest/iced/struct.Subscription.html
- **iced examples directory**: https://github.com/iced-rs/iced/tree/master/examples

### Key Crates
- **iced_color_wheel** (our HSV wheel): https://github.com/zhanjii/iced_color_wheel
- **rfd** (native file/message dialogs): https://docs.rs/rfd/latest/rfd/
- **tray-icon** (system tray): https://docs.rs/tray-icon/latest/tray_icon/
- **tokio** (async runtime): https://docs.rs/tokio/latest/tokio/
- **serde** (serialization): https://serde.rs/

### Troubleshooting
- **iced discussions** (Q&A, patterns): https://github.com/iced-rs/iced/discussions
- **crates.io** (dependency versions): https://crates.io/

## Model Preference

For complex tasks, prefer **the latest Opus model with extended thinking**:

- Opus requires less steering and produces better tool use than smaller models
- Despite being a larger model, it is often faster overall due to fewer retries and corrections
- Particularly effective for multi-file refactors, architectural decisions, and debugging complex issues
- For simple, well-defined tasks (typo fixes, single-line changes), Sonnet is sufficient
## Memory & Context Management

Claude Code has a persistent auto-memory directory for each project. Use it to build institutional knowledge across sessions.

### What to Save to Memory
- **Architecture decisions** — why you chose a pattern, framework, or library
- **Project conventions** — naming patterns, directory structure, import rules
- **Debugging insights** — recurring issues and their root causes (especially borrow checker patterns)
- **User preferences** — workflow habits, code style preferences, communication style
- **Key file paths** — entry points, config files, test locations

### How to Organize Memory
- `MEMORY.md` — main index (kept under 200 lines, always loaded into context)
- `architecture.md` — system design, data flow, component relationships
- `patterns.md` — recurring code patterns and conventions for this project
- `debugging.md` — solved problems and their solutions
- Link detailed files from MEMORY.md for quick navigation

### Rules
- Organize semantically by topic, not chronologically
- Update or remove memories that turn out to be wrong
- Do not duplicate what's already in CLAUDE.md — memory is for learned context
- Verify against project code before writing — don't save speculative conclusions
- When the user corrects something from memory, update the source immediately
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
