<!-- From: AGENTS.md -->
# Vitro Project Agent Guide

> **Maintenance note (2026-09-11)**: This is the only English document kept in the repository.
> It reflects the 2026-09-11 backend-only state and may lag behind the Chinese
> [`AGENTS.md`](AGENTS.md), which is the authoritative version (the docs overhaul of 2026-09-11 —
> archiving stale docs, rewriting core docs, dropping all other English documents —
> is recorded there). Translation of the Chinese docs is deferred; when in doubt, follow `AGENTS.md`.

## Project Overview

> **Repositioning (2026-09-11)**: Vitro has transitioned from a "cross-platform C IDE" into a **teaching C/C++ subset reference execution engine (white-box)** — this repository is backend-only (MIT license); the frontend is split out to the community and native mobile is dropped. See [`docs/current/01-定位与路线/后端定位与白箱计划.md`](docs/current/01-定位与路线/后端定位与白箱计划.md).
>
> **Frontend split executed (2026-09-11)**: `CideFlutter/`, the FRB bridge (`native/src/api/` + `frb_generated`), the web deploy workflow and all Flutter build scripts have been removed. The last complete pre-split state is preserved by tag `before-frontend-split` (`git checkout before-frontend-split -- CideFlutter` to recover).

Current architecture (one core, three exits):

- **Core**: Rust workspace compiler/VM (`native/`, 10 sub-crates); pipeline Lexer → Parser → TypeChecker → BytecodeGen → VitroVM
- **Exit 1**: C ABI (`native/src/capi/`) — first consumption path of `vitro_cli` and the shadow defenses (ctypes)
- **Exit 2**: wasm32 (smoke-tested: builds at 3.75MB with zero changes; C API full chain + safety checks work under wasm) — browser/white-box form
- **Exit 3**: `vitro_cli serve` JSON-lines session mode (id correlation / isomorphic error frames / `session.reset`; shares `native/src/session_api.rs` with capi) — headless interaction
- **No git commits without permission**
- **Honest records**: This project is a teaching C/CPP subset, with Clang as the standard. Any deviation between this project and the standard must be recorded.

## Tech Stack

| Layer | Technology |
|:------|:-----------|
| Native | **Rust 1.95.0**, Cargo, cdylib/staticlib/rlib |
| VM | Custom bytecode interpreter, 1 MB linear memory |
| Exits | C ABI (capi), wasm32 (wasm-bindgen), JSON-lines (serve) |

## Key Directories

```
native/crates/          Compiler/runtime sub-crates (crate modularization in progress)
  vitro_shared/          Shared basics: SourceLoc, ErrorCode
  vitro_ast/             AST nodes and type system
  vitro_lexer/           Lexer
  vitro_parser/          Parser
  vitro_cpp_frontend/    C++ frontend support
  vitro_typeck/          Type checker
  vitro_codegen/         Bytecode generator
  vitro_runtime/         VM runtime shared data (memory state, opcode/instruction, symbol table)
  vitro_vm/              VitroVM bytecode interpreter
native/src/compiler/    Remaining local modules: algorithm_detector, cfg, data_flow, intent (Rust)
native/src/unified/     Unified mode / time-travel engine (Rust)
native/src/engine/      Compiler pipeline and tools (Rust)
native/src/capi/        C API (exit 1, public API, ABI-versioned) (Rust)
native/src/session_api.rs Language-neutral session layer (shared by capi and serve; exits stay thin) (Rust)
native/src/flutter_bridge.rs Legacy session wrapper (currently consumed by vitro_cli; rename pending) (Rust)
native/src/diagnostics/ Structured diagnostics, auto-fix suggestions, knowledge graph, teaching reasoning (Rust)
templates/              Algorithm template sources (source.c + meta.yaml; kept pending community-frontend adoption)
docs/                   Design documents, incident reports
```

> The historical frontend `CideFlutter/` and the FRB bridge have been split out (tag `before-frontend-split`).

## Rust Migration Progress (Completed ✅)

| Phase | Module | Status |
|:------|:-------|:-------|
| Phase 0 | Rust skeleton + C API stubs + Session types | ✅ Done |
| Phase 1 | VM migration (VitroVM + host funcs) | ✅ Done |
| Phase 2a | Lexer | ✅ Done |
| Phase 2b | AST | ✅ Done |
| Phase 2c | Parser | ✅ Done |
| Phase 2d | TypeChecker | ✅ Done |
| Phase 2e | BytecodeGen | ✅ Done |
| Phase 2f | C API `vitro_compile_all` wiring | ✅ Done |
| Phase 3 | ~~C# frontend~~ → Flutter frontend end-to-end tests | ✅ Done |
| Phase 4 | Android target build (cargo-ndk) | ✅ Done |
| Phase 5 | Clean up legacy C++ / CMake files | ✅ Done |
| Phase 6 | Comprehensive review: compiler warning cleanup + security hardening + test coverage expansion | ✅ Done |
| Phase 7 | Desktop memory leak fixes + sizeof/scanf subset expansion | ✅ Done |
| Phase 8 | Full `float` pipeline support (Lexer→Parser→TypeChecker→BytecodeGen→VM) + diagnostics expansion | ✅ Done |
| Phase 9 | Flutter frontend built from scratch: IDE UI + editor + debug panel + algorithm visualization | ✅ Done |
| Phase 10 | Memory-mapped Canvas + algorithm visualization event FRB integration + interaction enhancements | ✅ Done |
| Phase 11 | Code review fixes + engineering standards (`rustfmt.toml`/`CHANGELOG.md`) + full Flutter frontend modularization | ✅ Done |
| Phase 12 | Full `union` pipeline support (Lexer→Parser→TypeChecker→BytecodeGen→VM) + `sizeof(union U)` | ✅ Done |
| Phase 13 | Unified mode / time travel: VM snapshot/restore, checkpoint manager, Seek progress bar, exception rollback | ✅ Done |
| Phase 14 | Heap memory visualization enhancement: malloc line tracking, external fragmentation visualization, leak detection reports | ✅ Done |
| Phase 15 | Pointer tracking animation: `PointerSnapshot` four states (Valid/Freed/Null/Dangling) real-time arrow rendering | ✅ Done |
| Phase 16 | Algorithm step semantic annotation: 27 predefined algorithm step templates, runtime teaching description inference | ✅ Done |
| Phase 17 | Code template parameterization + interactive tutorial: parameter placeholders, `TemplateTutorialPanel` line-by-line guidance | ✅ Done |
| Phase 18 | 6-04 carpet review: P0 soundness fixes, VM optimization, DRY refactoring, clippy 0 warnings | ✅ Done |
| Phase 19 | Use-After-Free / Double-Free runtime detection: `freed_logs` instruction-level checks, knowledge cards E3060/E3061 | ✅ Done |
| Phase 20 | Cognitive reasoning P0: `TraceAnalyzer` trace slicing + 5-class Trap root-cause inference, `RootCauseHint` | ✅ Done |
| Phase 21 | Cognitive reasoning P1: `MisconceptionPattern` 6-pattern detection + `LearningPath` recommendation engine | ✅ Done |
| Phase 22 | Cognitive reasoning P2: `KnowledgeGraph` 24 concept nodes + 30+ relationship edges, `ConceptGraphView` | ✅ Done |
| Phase 23 | Cognitive reasoning P3: `ControlFlowGraph` + `DataFlow` + `IntentInference` code intent inference | ✅ Done |
| Phase 24 | Semantic intelligent completion v2: `CompletionEngine` five context-aware completion types | ✅ Done |
| Phase 25 | Template JIT (Trace-based Loop Accelerator): hot-loop trace recording + pre-optimized function pointer sequence | ✅ Done |
| Phase 26 | Flutter Bridge communication optimization: Stream mode, differential encoding `StepPayloadDelta`, symbol table dedup | ✅ Done |
| Phase 27 | Data structure syntax expansion P0+P1: array decay, `unsigned` full pipeline, `const`, `extern`, VLA full pipeline | ✅ Done |
| Phase 28 | CLI debugging tool `vitro_cli`: `compile`/`run`/`step`/`unified`, supporting stdin pipes for quick testing | ✅ Done |
| Phase 29 | Bytecode Libc productization: build-time precompilation + fixed index segment + ctype/abs via bytecode path | ✅ Done |
| Phase 30 | P0 syntax expansion: generic comma operator, Designated Initializer, offsetof + regression fixes | ✅ Done |
| Phase 31 | C++ extension P0: Lexer/Parser/AST keyword and node expansion | ✅ Done |
| Phase 32 | C++ extension P1: TypeChecker classes/inheritance/template monomorphization | ✅ Done |
| Phase 33 | C++ extension P2: BytecodeGen virtual functions/this pointer/constructors/destructors | ✅ Done |
| Phase 34 | C++ extension Stage 0.5: container收口 (list<int>/vector<char>/sort_int), C++ three-tier CI inclusion, CPP_FAILURES.md | ✅ Done |
| Phase 35 | C++ extension Stage 2: stack object RAII (default ctor auto-call / scope exit / return / break / continue auto-dtor) | ✅ Done |
| Phase 36 | C++ extension Stage 3: `new[]/delete[]` element construction/destruction (base[-4] stores count, reverse dtor, temp slot expanded to 4) | ✅ Done |
| Phase 37 | C++ extension Stage 4: reference declaration and basic semantics (`int& r = x` full pipeline; `T&` parameter/return; reference auto-deref; implicit address-of; returning reference lvalue recognition) | ✅ Done |
| Phase 38 | C++ extension Stage 5: implicit move constructor auto-generation (class with pointer/resource fields auto-generates `__ctor__{Class}__move`; `std::move` initialization calls move ctor; source pointer fields nulled to prevent double-free) | ✅ Done |
| Phase 39 | C++ extension Stage 6: simplified `unique_ptr<T>` dogfooding + constructor initialization syntax `Type name(args);` + constructor overload/implicit default ctor | ✅ Done |
| Phase 40 | C++ extension M6: test defense wrap-up — 59 new C++ E2E regression cases (core language / container algorithms / teaching OJ), `test_vitro_e2e_cpp` in CI, Golden generated by Clang++ | ✅ Done |
| Phase 41 | C++ built-in container layout decoupling: `.cpp` interface declarations as single source of truth + JSON loader, zero Rust hard-coding | ✅ Done |
| Phase 42 | P0 syntax/stdlib expansion + code review report progress + performance optimization ([Unreleased]) | 🚧 In progress |

## Test Defenses

Vitro adopts **five layers of collaborative test defenses**. Core philosophy: *tests are not for boasting pass rates, but for honestly discovering potential problems*. Any failure must be recorded truthfully; modifying test expectations to beautify data is prohibited.

```
┌────────────────────────────────────────────────────────────────┐
│  Defense 5: CI integration and consistency monitoring (Phase F) │
│  └─ Run all defenses automatically on PR; cross-validate *_FAILURES.md with test results │
├────────────────────────────────────────────────────────────────┤
│  Defense 4: Fuzz stress testing (Phase E)                       │
│  └─ Random memory state + random standard library call sequences, verify safety detection does not leak │
├────────────────────────────────────────────────────────────────┤
│  Defense 3: Three-tier contract verification (Phase A~C)        │
│  ├─ 3a Host Contract: Rust unit tests directly verify Host Func boundary behavior │
│  ├─ 3b Bytecode Self-Consistency: C source → Clang vs Vitro self-hosting │
│  └─ 3c Differential Stress: cross-compare multiple implementations of the same feature │
├────────────────────────────────────────────────────────────────┤
│  Defense 2: K&R real-program regression (existing) + LeetCode (started, Phase 4) │
│  └─ K&R verifies "can real-world code run"; LeetCode easy problems gradually being filled │
├────────────────────────────────────────────────────────────────┤
│  Defense 1: Shadow Verification (existing)                      │
│  └─ Verify "behavior consistency with Clang"                    │
└────────────────────────────────────────────────────────────────┘
```

### Defense 1: Shadow Verification

The same C source is compiled and executed by both **Clang** and **Vitro**, and stdout outputs are compared for exact match. Golden outputs must come from Clang, never from Vitro itself.

- **Coverage**: 321 Baseline cases + 82 template-generated cases + 81 K&R cases + 138 LeetCode problems + 14 gap cases (636 C Shadow Verification cases total, 617 exact match + 16 vitro_better + 3 known_issue, counting match + vitro_better + known_issue; re-measured 2026-09-11); 100 C++ cases (C++ Shadow Verification, 98 match + 2 recorded `clang_compile_fail`: `cpp_vitro_vec_class` / `cpp_vitro_list_class` cannot be compiled directly by Clang++ because they use Vitro built-in containers; measured 2026-06-28, re-verified 2026-09-11)
- **stdin (`.in`) injection (added 2026-09-11)**: a case may ship a sibling `.in` file; Clang and Vitro are fed the **same bytes** (the cache key includes the real stdin). Previously the defense always ran in batch mode with no stdin — 29 `.in` files under `knr/` were never used, producing **false matches** where "no input on both sides" happened to agree. Enabling it immediately exposed a newline-dropping defect in input injection (`getchar()` could never read `'\n'`; 19 `output_gap` cases), fixed by unifying on `RuntimeState::split_stdin`
- **Output channels (E-P1-5, 2026-09-11)**: comparison reads the engine's **program-stdout channel only** (capi `vitro_get_program_output*`, ABI 1.1.0) — engine notes ("程序运行完成，返回值：N", leak reports, teaching warnings) and stderr each have their own channel. **Drivers must not regex-clean output any more**: the dozen or so scattered cleaning rules disagreed with each other and deleted real output when a teaching program printed look-alike text (false-positive `output_gap`); all of them were removed. The single read entry point is the driver's built-in structured-channel helper (`readChannel`/`ptrToGoString` in `scripts/shadow_verify.go`; the former Python companion `vitro_output.py` was retired with the main driver); a DLL missing the new symbols fails fast instead of falling back to cleaning. Regression case `baseline/engine_note_lookalike.c` pins this contract
- **Drivers**: `go run ./scripts/shadow_verify` (C main driver, final step of the D5 language migration on 2026-09-13: took over CI after full-dimension dual-run parity with the Python version, `shadow_verify.py` retired. Shape: two-stage pipeline — Clang side concurrent (`--jobs N`, 0 = auto min(CPU,16)), Vitro side serialized under a mutex (DLL is not thread-safe, empirically); measured ~26s for a cold full re-run of 663 cases, ~5s warm. `--refresh-clang` forces a full re-run (nightly CI anti-drift); `--rebuild` rebuilds when the release DLL is older than engine sources; the Clang result cache uses a Go-owned schema (`go1`) co-existing with legacy Python entries in the same directory with disjoint key spaces; transient environment failures (timeout / spawn failure / 0xC0000005 image crash) are retried and never cached), `go run ./scripts/shadow_verify_cpp` (C++ driver since 2026-09-12, D5 migration step 1: Clang 16-way concurrency, 24.75s → 5.2s; self-managed `.shadow_cpp_tmp/` working directory)
- **Reports**: `native/tests/shadow_verification/reports/`

### Defense 2: K&R Real-Program Regression (existing) + LeetCode (planned)

Collect real teaching/competition code as end-to-end regression cases to verify "can real-world code run".

- **Baseline**: `native/tests/cases/baseline/` (314 cases, all green)
- **K&R**: *The C Programming Language* exercises (76 cases, 76 green, 0 known failures)
- **Template Generated**: algorithm template batch generation (82 cases, 78 green, 4 known failures)
- **LeetCode**: Phase 4 + Phase 5 fully implemented; current 138 problems all pass, see `native/tests/LEETCODE_FAILURES.md`
- **Reports**: `native/tests/TEST_REPORT.md`, `KR_FAILURES.md`, `E2E_FAILURES.md`, `LEETCODE_FAILURES.md`

### Defense 3: Three-Tier Contract Verification

The same feature may simultaneously exist as VM Builtin, Rust Host, and Bytecode Libc implementations, which must be independently verified for consistency.

| Sub-layer | Goal | Key files |
|:----------|:-----|:----------|
| **3a Host Contract** | Verify Layer B Host Func boundary conditions and safety injection (UAF/Double-Free/Buffer Overflow) | `native/tests/host_contract_tests.rs` |
| **3b Bytecode Self-Consistency** | Can Vitro compiler + VM correctly compile and run "its own standard library" | `native/tests/bytecode_libc_consistency.rs` + `bytecode_libc_consistency/src/*.c` |
| **3c Differential Stress** | Cross-validate Host and Bytecode versions of the same feature; results must always match | `native/tests/differential_stress.rs` |

- **Failure records**: `HOST_CONTRACT_FAILURES.md`, `BYTECODE_LIBC_FAILURES.md`, `DIFFERENTIAL_FAILURES.md`

### Defense 4: Fuzz Stress Testing

Use a **deterministic RNG** to generate random memory states and random standard library call sequences, verifying that safety detection does not leak or crash.

| Scenario | Coverage |
|:---------|:---------|
| **Fuzz A** | malloc/free/realloc random sequences + UAF/Double-Free detection verification |
| **Fuzz B** | strcpy/strcat/strncpy/memcpy/memmove + Buffer Overflow (E3070) |
| **Fuzz C** | printf/scanf/getchar/putchar random formats and inputs |
| **Fuzz D** | Mixed malicious sequences (memory/strings/IO/rand cross) |
| **Fuzz E** | Random allocation + partial release, verify leak report accuracy |

- **Driver**: `cargo test --test fuzz_stress_test`
- **Records**: `native/tests/FUZZ_FAILURES.md`

### Defense 5: CI Integration and Consistency Monitoring

`.github/workflows/ci.yml` automatically runs all defenses above on every Push/PR, and executes `scripts/ci_three_tier_check` (Go driver since 2026-09-18; the `.py` was retired the same day) for consistency checks:

- If a test marked `KNOWN_FAILURE` in `*_FAILURES.md` now passes → **error prompting document update**
- If a test fails but has no corresponding record in the document → **error prompting adding a record**
- Generates `reports/three_tier_report.md` as a CI artifact upload

---

## Coding Conventions

### Rust (native)
- AST uses enum instead of C++ polymorphic class hierarchy: `Expr` / `Stmt` enums + `Box<Expr>` / `Vec<Box<Expr>>`
- `SourceLoc` has `Copy` derive (two `i32`s, pass-by-value has no overhead)
- Parser zero-progress guard: `if pos_ == checkpoint { self.advance(); }`
- Error handling: no panic; collect into `Vec<Error>` and return uniformly
- Borrow checker conflict resolution pattern: clone data first, then call methods requiring `&mut self`

> No Dart/Flutter code remains in this repository after the frontend split; see tag `before-frontend-split` for historical frontend conventions.

## C Teaching Subset Overview

The C teaching subset supported by this project covers **Phase 1 ~ Phase 5+** capabilities (including comma operator, Designated Initializer, `offsetof`, VFS file I/O, etc.). Detailed spec: [`docs/current/03-语言子集/C语言子集规范.md`](docs/current/03-语言子集/C语言子集规范.md). Core support includes:

**Data types**: `int`, `char`, `float`, `double`, `unsigned`, `_Bool`/`bool`, `int*`, `char*`, `float*`, `double*`, `int[]`, `char[]`, `double[]`, `struct` (including return by value), `union`, `enum`, `typedef`

**Arrays**: fixed-size arrays (1D/multi-dimensional), **VLA variable-length arrays** (`int a[n]` / `int a[n][3]`, local scope, runtime stack allocation), array/string initializer lists, array parameter decay semantics

**Pointers**: address-of `&`, dereference `*`, pointer arithmetic (automatic step scaling), **multi-level pointers** (`int**`, `struct Node**`), explicit cast `(int*)p`, function pointers (including indirect calls, struct members, typedef)

**Statements**: variable declarations (including multiple variables, block scope), `if/else`, `while`, `do...while`, `for` (C99-style variable declaration), `switch/case/default`, `break`, `continue`, `return`

**Expressions**: arithmetic, comparison, logical (short-circuit evaluation), bitwise `& | ^ ~ << >>`, assignment (including compound assignment; pointer `+=` / `-=` with integer is supported with pointee-size scaling, and `void*` arithmetic uses 1-byte steps as a GCC/Clang extension), ternary `?:`, array index, function call, `&`, `*`, struct access `.` / `->`, `++` / `--`, `sizeof`

**Functions**: definition/call/recursion/forward declaration, **functions returning struct by value** (Hidden Return Pointer ABI)

**Memory**: `malloc`/`free`/`realloc`

**I/O**: `printf`/`scanf`/`sprintf`/`snprintf`/`sscanf`/`fprintf`/`fgets`/`fputs`/`puts`/`getchar`/`putchar`/`ungetc`; VFS sandbox file I/O: `fopen`/`fread`/`fwrite`/`fclose`/`feof`/`fgetc`/`fputc`/`fseek`/`ftell`/`rewind`

**Strings**: `strlen`, `strcpy`, `strncpy`, `strcmp`, `strncmp`, `strcat`, `strncat`, `memcpy`, `memmove`, `memcmp`, `strchr`, `strrchr`, `strstr`, `memchr`, `strdup`, `atoi`

**Math**: `sin`/`cos`/`tan`/`sqrt`/`pow`/`atan`/`log`/`log10`/`exp`/`fabs`/`abs`/`ceil`/`floor`/`round`/`fmod` (via `libm`, `double` precision)

**Type system**: `typedef`, `sizeof`, `const`, `static` (local+global+function), `extern`, `volatile`, `restrict`, `inline`, `register`, `auto`

**Headers**: `#include <stdio.h>` / `<stdlib.h>` / `<ctype.h>` / `<math.h>` / `<string.h>` stub declaration loading

**Others**: `rand`/`srand`, `memset`, `exit`, `qsort`, `calloc`, `bsearch`, `atof`/`atol`, `#define` macros (object macros / parameterized macros / nested calls)

**Character classification**: `isdigit`/`isalpha`/`islower`/`isupper`/`isalnum`/`isspace`/`isprint`/`iscntrl`/`isxdigit`/`tolower`/`toupper` (`ctype.h`, some paths go through Bytecode Libc)

**C++ classes and templates (Phase 31+)**: `class`, member access control, `this` pointer, virtual functions, template class monomorphization, stack object RAII (auto ctor/dtor), constructor initialization syntax `Type name(args);`, implicit default/move constructors, `std::move`, simplified `unique_ptr<T>` dogfooding (construction/`get`/`release`/`reset`/dtor/ownership transfer)

**Explicitly not supported**: bitfield, global VLA, full preprocessor (only `#define` constant macros + `#include` standard library stubs)

**C++ subset boundaries (honest record)**: class types as template arguments for built-in containers such as `vector<T>` / `list<T>`, non-type template parameters, nested class `Outer::Inner` instantiation, const reference parameters, default arguments, and user-defined copy constructors are not yet supported (recorded 2026-06-26). These features are outside the current Vitro C++ teaching subset (Stages 0~6) and will be extended as teaching needs evolve.

## Known Limitations

### Currently Not Supported
- ~~**Parameterized macro calls followed by semicolon**~~ — **Fixed (extension, 2026-06-25)**. During parametric macro expansion, `vitro_lexer` dynamically wraps a brace-enclosed macro body as `do { ... } while(0)` when the call is immediately followed by a semicolon, so `SWAP(int,x,y);` now parses correctly inside `if/else` and similar contexts. Regression test added at `end_to_end_extra_test::test_e2e_parametric_macro_swap_semicolon`.
  - ⚠️ **Behavioral difference from Clang**: Clang rejects `if (...) { ... }; else ...` with "expected expression"; Vitro supports this common teaching idiom via automatic wrapping. For strict Clang compatibility, use `do { ... } while(0)` in the macro body.
- ~~**VLA bounds checking**~~ — **Fixed (2026-06-25)**. `gen_index` now emits runtime bounds checking when the first dimension of a VLA is a variable expression: new `TrapBoundsVla` opcode evaluates the VLA dimension expression at index time and compares the index against the runtime bound in the VM. Regression case added at `baseline/vla_bounds.c`. VLA parameters that have decayed to pointers (e.g. `void f(int n, int a[n])`) still cannot be checked because their bound is not available at the use site.
- ~~**`#include` non-standard library paths**~~ — **Fixed (2026-06-25)**. `#include "header.h"` now loads custom headers relative to the source file directory; standard libraries still use stubs. Regression cases added at `baseline/include_custom_header.c` and `include_custom_header.h`. Absolute paths, system include search paths (`<>` non-standard libraries), and recursive includes remain future work.
- ~~**`va_list` / `va_start` / `va_arg` / `va_end`**~~ — **Fixed (2026-06-25)**. Custom variadic functions now work end-to-end: `va_list` is modeled as `char*`, `va_start`/`va_arg`/`va_end` are implemented via internal host functions, and `va_arg` reads by dereferencing the address according to the target type. Common types such as `int`, `double`, and `long long` are supported (following C default argument promotions: `float` → `double`, `char` → `int`). Regression case added at `baseline/variadic.c`.
- **Global VLA** — variable-length arrays in global/static scope are prohibited by the C99 standard itself (Clang reports "variable length array declaration not allowed at file scope"); Vitro intentionally does not support this.
- **VFS text mode newline conversion (fixed)** — as of 2026-06-15, Windows text-mode newline conversion is fully implemented: in `"r"`/`"w"` mode, `\n` is expanded to `\r\n` on write and collapsed to `\n` on read; `fseek`/`ftell` distinguish logical/physical cursor to match Windows CRT behavior. `vfs_io_extensions.c` and `file_fread.c` are restored to matching.

### Known Behavioral Differences Between Vitro and Clang (Honest Records)

The following inconsistencies between Vitro and Clang were discovered during LeetCode defense filling:

- ~~**Compound side-effect array indexing**~~ — **Fixed (2026-06-25)**. Root cause: `gen_mem_inc_dec` (pre-/post-increment/decrement memory operation) and `gen_assign` reused the same temporary slot (`temp_slot0`) for Index assignment, so the side effect of the right-hand index expression overwrote the left-hand address temporary. The fix uses `temp_slot3` to save the new value in `gen_mem_inc_dec`; regression case added at `baseline/side_effect_index.c`.
- ~~**Function returning `double` value is incorrect**~~ — **Fixed (2026-06-25)**. Root cause: the `return` statement did not insert an implicit cast for the return expression, so `return 2.5;` (where `2.5` is parsed as a `float` literal) generated `PushConstF` instead of `PushConstD` when the function return type was `double`. The fix calls `insert_implicit_cast` after `check_assignable` in `vitro_typeck::decl.rs`; regression case added at `baseline/float_func_return.c`. `lc_4.c` has been restored to the original `double` return implementation.
- ~~**`scanf` `%s` format specifier not supported**~~ — **Fixed (2026-06-25)**. `host_scanf_n`/`host_sscanf` in `crates/vitro_vm/src/host/io.rs` now handle the `'s'` specifier: skip leading whitespace, read a sequence of non-whitespace characters, and write them to the target buffer terminated by `'\0'`. Regression case added at `baseline/scanf_string.c` (based on `sscanf` to avoid uncontrollable input between Shadow Verification cases).
- ~~**`fputs(str, stdout)` produces no output**~~ — **Fixed (2026-06-19)**. `host_fputs` in `crates/vitro_vm/src/host/file.rs` now recognizes the lexer-predefined `stdout`(1)/`stderr`(2) macro fds and appends the string to the program output; writing to ordinary `FILE*` streams remains unchanged. (Since E-P1-5 these land in the structured `stdout` / `stderr` channels.)
- **`fprintf` to a custom `FILE*` does not reach the file** — **Pre-existing deviation (recorded 2026-09-11, audited alongside E-P1-5)**. `host_fprintf_n` does not interpret its `stream` argument: `stderr` is now correctly routed to the stderr channel, but `fprintf(fp, ...)` (with `fp` from `fopen`) is emitted as stdout instead of being written to the VFS file (unlike Clang). Teaching code mostly pairs `fprintf` with `stderr`, so none of the 633 Shadow cases hit this; use `fputs`/`fwrite`/`fputc` when writing to files.
- ~~**`FILE*` still reported as memory leak after `fclose`**~~ — **Fixed (2026-06-25)**. Root cause: `host_fclose` only closed the VFS file descriptor but did not release the 4-byte `FILE*` struct allocated on the VM Heap by `host_fopen`. The fix adds `MemoryState::free_region` and calls it from `host_fclose`; non-heap streams such as `stdout`/`stderr` are safely ignored when no matching region is found. Regression case added at `baseline/fclose_leak.c`.
- ~~**VLA bounds checking missing**~~ — **Fixed (2026-06-25)**. `gen_index` now emits runtime bounds checking for VLAs whose first dimension is a variable expression via the new `TrapBoundsVla` opcode; the VM compares the index against the runtime-evaluated bound. Regression case added at `baseline/vla_bounds.c`. VLA parameters decayed to pointers are still unchecked.
- ~~**Parameterized macro calls followed by semicolon**~~ — **Fixed (extension, 2026-06-25)**. `vitro_lexer` dynamically wraps brace-enclosed parametric macro bodies as `do { ... } while(0)` when the call is followed by a semicolon, allowing `SWAP(int,x,y);` to parse inside `if/else`. Regression test added at `end_to_end_extra_test::test_e2e_parametric_macro_swap_semicolon`.
  - ⚠️ **Behavioral difference from Clang**: Clang rejects the unwrapped `{ ... };` form, while Vitro supports it as a teaching convenience.
- **Pointer compound assignment `+=` / `-=`** — **Supported (2026-06-28)**. `int* p; p += n;` and `p -= n;` are supported end-to-end with pointee-size scaling; `void* p; p += n;` uses 1-byte steps as a GCC/Clang extension. Function pointer arithmetic, pointer-pointer `+=` / `-=`, and other compound operators (`*=`, `/=`, etc.) remain errors. Regression cases added at `baseline/pointer_add_assign*.c`.
  - ⚠️ **Behavioral difference from Clang**: `void*` arithmetic is a GCC/Clang extension and is undefined in strict C; prefer concrete pointer types in teaching. The value returned by a compound assignment expression is an rvalue pointer in Vitro, differing from the C standard lvalue semantics, though this is rarely relied upon in teaching code.

> Historical feature details and bug-fix records are in [`CHANGELOG.md`](CHANGELOG.md) and [`docs/current/03-语言子集/C语言子集规范.md`](docs/current/03-语言子集/C语言子集规范.md).

## Build Commands

```bash
# Build the Rust engine (Debug / Release)
cd native && cargo build            # Debug
cd native && cargo build --release  # Output: native/target/release/vitro_native.dll

# Build the CLI debug tool
cd native && cargo build --release --bin vitro_cli

# Build the wasm32 exit (browser/white-box form; smoke-tested at 3.75MB)
cd native && cargo build --target wasm32-unknown-unknown --release

# Tests and static checks
cd native && cargo test --workspace --all-features
cd native && cargo clippy --workspace --all-targets --all-features -- -D warnings

# Shadow defenses (C / C++)
go run ./scripts/shadow_verify
go run ./scripts/shadow_verify_cpp

# serve protocol smoke
cargo build --bin vitro_cli && go run ./scripts/serve_smoke
```

> Historical frontend builds (Flutter / Android / iOS) were removed with the frontend; scripts live under tag `before-frontend-split`.

## Debugging Tips

### Native Layer Debugging (Rust)
1. Project properties → Debug → **Enable native code debugging**
2. Set breakpoints in `vitro_compile_all` / `vitro_run` in `native/src/capi/mod.rs`
3. PDB warning (`apphost.pdb` missing) can be safely ignored
4. Frontend-less debugging: `vitro_cli step <file>` interactive stepping (`p` prints variables / `o` prints output), or `vitro_cli serve` JSON-lines sessions

### Memory Leak Localization
- Parser infinite loop symptom: memory grows slowly and continuously (~100MB/s), AST nodes or error messages keep accumulating

## CLI Debugging Tool

The project provides an independent command-line debugging tool `vitro_cli` that operates the Rust backend compiler/VM directly (no frontend dependency; the first entry point for headless debugging).

### Build

```bash
cd native && cargo build --release --bin vitro_cli
```

### Commands

| Command | Description |
|:--------|:------------|
| `compile <file>` | Compile and display diagnostics (error codes + fix suggestions) |
| `run <file>` | Compile and run at full speed |
| `step <file>` | Interactive step debugging (supports `p` print variables, `o` print output, `r` run to end, `q` quit) |
| `unified <file>` | Unified mode (time-travel engine) batch execution and summary output (supports `--max-steps <n>`) |
| `export <file1> [file2 ...] -o <out.json>` | Precompile to bytecode artifact (multiple files + `--builtin-libc` option) |

### Options and Special Filenames

- `-i <file>`: read standard input from file (multi-line input for `scanf`/`fgets`)
- `--max-steps <n>`: maximum execution steps allowed in unified mode (default 100_000), for long programs or performance baseline testing
- `-`: read source code from standard input for quick code snippet testing

### Quick Test Examples

```bash
# Pipe directly
echo '#include <stdio.h>
int main() { printf("hello\n"); return 0; }' | vitro_cli run -

# here-document compile
vitro_cli compile - <<'EOF'
#include <stdio.h>
int main() {
    int a = 10, b = 20;
    printf("%d\n", a + b);
    return 0;
}
EOF

# Run with input file
vitro_cli run sum.c -i input.txt

# Unified mode execution
vitro_cli unified hello.c

# Unified mode with increased step limit (for long programs or performance baseline)
vitro_cli unified long_sort.c --max-steps 500000

# Precompile bytecode artifact (with Bytecode Libc)
vitro_cli export main.c libc_helper.c -o bundle.json --builtin-libc
```

Full documentation: [`docs/current/02-构建与上手/CLI使用手册.md`](docs/current/02-构建与上手/CLI使用手册.md).
