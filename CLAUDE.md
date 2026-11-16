# Escape — conventions for Claude Code

Dynamic Island for Windows on Tauri 2. **PLAN.md is the source of truth**: goals, architecture, stages, acceptance criteria.

## Rules (from PLAN.md §0)

- Work strictly stage by stage; start the next stage only after the user accepts the current one.
- At the start of a stage: re-read its section, list tasks, ask about anything UX-relevant that is unclear.
- One logical step = one commit, Conventional Commits (`feat(media): …`, `fix(window): …`, `chore: …`).
- At the end of a stage: tick `[x]` in PLAN.md, run all checks below, give the user the manual Windows checklist and stop.
- No dependencies outside PLAN.md §2 "Стек" without a short justification to the user.
- If a task turns out notably harder than planned (Windows limitation etc.), stop and propose options — never silently simplify.

## Stack

Tauri 2 · Rust stable (edition 2021) · `windows` crate · React 19 + TS strict + Vite · zustand · motion · Tailwind v4 · vitest + Testing Library.

## Layout

- `src/` — frontend. `app/` island root + state machine, `features/<name>/`, `ipc/` typed `invoke`/`listen` wrappers, `i18n/ru.ts` (**all UI strings**), `styles/`.
- `src-tauri/src/` — Rust. `lib.rs` assembles plugins/commands, `error.rs` holds `AppError` (serialized as `{kind, message}`), one module per domain (`window/`, `system/`, `media/`, `clipboard/`, `launcher/`, …).

## Code rules

- All WinAPI/WinRT code is in Rust, in `#[cfg(windows)]` modules. The frontend knows nothing about Windows APIs.
- Every command returns `AppResult<T>`; IPC types are mirrored in `src/ipc/*.ts`. Event names: `domain://event`.
- No `unwrap()`/`expect()` in production code (clippy denies them; allowed in tests). Every `unsafe` block has a `// SAFETY:` comment.
- Rust modules expose a small public API (2–5 functions). Heavy work off the main thread; no polling faster than 16 ms.
- No network requests except update checks, opening a browser search, and AI chat messages that the user sends to the provider they chose: `api.anthropic.com` (Claude) or `generativelanguage.googleapis.com` (Gemini) (stage 6, approved by the user). API keys stay in Rust (Windows Credential Manager).

## Checks (all must pass)

```bash
npm run lint        # eslint + prettier --check
npm run typecheck
npm test
cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

Run the app: `npm run tauri dev`.
