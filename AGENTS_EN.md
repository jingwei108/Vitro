# Vitro Agent Guide (English Summary)

> Details are maintained in Chinese; this file is routing-only.

## Dual-zone repository (since 2026-09-18 — MoonBit migration, strangler pattern)

- **MoonBit active zone** — `moonbit/` (created at milestone S1). All new development happens here. Rules: `docs/current/01-定位与路线/MoonBit迁移总计划.md` (master plan) and `MoonBit迁移第一阶段计划.md` (phase-1 plan).
- **Rust frozen oracle zone** — `native/`, `scripts/`, `.github/`; frozen at tag `rust-oracle-freeze`. Only the phase-1 whitelist (plan §2: P1–P7 / U1 / U2), security fixes, and defense-line maintenance. Full operating manual (build / test defenses / coding conventions / subset overview / known Clang divergences / debugging / CLI): [`native/AGENTS.md`](native/AGENTS.md) — **read on demand only**, to avoid context pollution and cross-zone hallucination.

**MoonBit package naming (decided 2026-09-19)**: module name = `vitro/engine` (mooncakes owner `vitro`); every package is `vitro/engine/<pkg>` (e.g. `vitro/engine/diag`) in `moon.pkg` imports, generator-script comments, and `.mbti` interfaces. Plan documents may abbreviate to `vitro/<pkg>`; code and config must not.

## Global disciplines (both zones)

Chinese output required; no git commits without permission; **measurement over speculation with a single source of truth** (verify by running commands / reading code; reports are leads, not evidence); honest recording of any divergence from Clang; red→green discipline (a failing test case precedes every fix, commit messages reference the case name; guard scripts must be proven able to fail — J9); never reference `docs/archive/`; new docs go under `docs/current/<category>/` with Chinese filenames and a synced `docs/README.md` index; judgment scripts default to Go with zero third-party dependencies and externalized JSON rules.

## Current phase & archive

Phase 1 (S0.5 Rust stopgap batch + S1 foundation slice) — see the phase-1 plan. After full switchover the Rust zone is **deleted, not moved**; the archive is tag `rust-oracle-freeze` + git history (16 exploration docs live in commit `917251e`).
