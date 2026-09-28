# Personal Mac Voice Input Implementation Plan

> **For agentic workers:** Use superpowers:subagent-driven-development for implementation and task review. The user has approved development; proceed without further design approval.

**Goal:** Ship a personal Mac build with toggle recording and a dependable copyable result window whenever dictation cannot safely reach an input.

**Architecture:** Preserve OpenTypeless's pipeline, model providers, dictionary, history and capsule. Add a focused-input adapter, final-output gate, backend result store and dedicated result window; keep macOS insertion atomic. Isolate personal-build app identity and turn off upstream automatic updates.

**Tech Stack:** Rust, Tauri 2, React 19, TypeScript, Vitest, macOS Accessibility.

**Spec:** docs/superpowers/specs/2026-09-28-personal-mac-voice-design.md

## Global Constraints

- Personal use, macOS only, cloud processing with the user's own configurable API keys.
- First press starts, release continues, second press stops; Escape cancels.
- No writable/known/stable destination or any insertion failure means full-text result window with Copy.
- Result persists until explicit dismissal and survives renderer startup; no lost result between recordings.
- Chinese UI, faithful cleanup, local history and dictionary; retain existing provider configuration.
- Separate name Voice Input and identifier com.local.voiceinput.personal; disable upstream updates/deep-link ownership; retain MIT notices.
- No publishing, pushing, installation into /Applications, embedded credentials, or real cloud calls without configured keys.

## Task 1: Complete personal dictation flow

**Files and responsibilities:**
- src-tauri/src/hotkey.rs and native_hotkey.rs: reuse toggle mode; prevent repeated keydown/busy presses from double submitting.
- src-tauri/src/output/focused_input.rs (new): classify and capture current macOS focused editable element; expose safe destination checks, with bounded native calls and correct CF ownership.
- src-tauri/src/commands/dictation_result.rs (new): backend-owned recoverable pending result, show/copy/dismiss commands, explicit session identity.
- src-tauri/src/pipeline.rs: snapshot destination at recording time, gate final text output, surface no-target/changed-target/output-error/LLM-raw-fallback result.
- src-tauri/src/lib.rs, commands/mod.rs, tauri.conf.json, capabilities/default.json: register state/commands/window and use separate personal app identity.
- src/components/DictationResult/ (new), src/App.tsx, src/lib/tauri.ts: renderer-ready payload retrieval, visible/selectable complete text, Copy feedback and explicit Close.
- src-tauri/src/storage/mod.rs, src/stores/appStore.ts, src/components/Onboarding/, existing updater/deep-link initialization and localization files: personal defaults and BYOK setup, keep voice input configuration functional.
- PERSONAL_USAGE.zh-CN.md (new): configuration, use and validation limits.

**Interfaces:** Backend result payload serializes session id, text and human-readable reason code. Snapshot and dismiss commands must act on an explicit session id; stale actions cannot delete a fresh result. Keep payload owned in backend, not only transient events. A failed copy returns an error and retains the payload. Focus classification must distinguish editable, not editable and unknown, with unknown routed to popup. The implementer chooses concrete Rust type names after reading adjacent code; record them in the report.

- [ ] Read the spec and relevant existing tests. Inspect all ordinary output paths and the existing foreground-app guard. Confirm current macOS default is already toggle before changing defaults.
- [ ] Write behavior tests first. Include keydown/release/second keydown; a held key repeat; busy press; writable field vs no field; unknown permission/read-only/secure fields; changed destination within the same app; failed insertion; copy failure; renderer loading after result; stale close against a newer result; consecutive results.
- [ ] Run each relevant test group and record meaningful red failures. Pure output-policy tests should call production functions with literal fixtures rather than grep source. UI tests must render the actual result component and mock only the native IPC/clipboard boundary.
- [ ] Implement the focused-input adapter and gate ordinary dictation output. Keep pre-existing provider/voice action behavior outside scope. Disable macOS streaming insert so final output follows the gate even when a setting is imported.
- [ ] Implement backend result retention and the dedicated frontend window. Ensure a popup shown after failed output cannot itself become the destination for that same output.
- [ ] Make personal defaults and app identity concrete; use BYOK without requiring upstream account sign-in. Retain settings for all existing provider selection and custom endpoint support.
- [ ] Run focused frontend tests, all frontend tests, npm run build and npm run lint. Run cargo tests covering modified Rust behavior once the controller's isolated Rust toolchain is available. Record exact commands and output counts.
- [ ] Write the Chinese usage guide, self-review the diff, then commit only owned files. Report any remaining upstream baseline failures separately with evidence.

## Task 2: Review, build and package

**Files:** deliverable app/zip in ../, test/build logs in ../../work, review report in this plan's SDD workspace. Do not change source during packaging unless review identifies a bug; fixes return to the implementer.

- [ ] Generate a diff review package from base commit 842f278 to the implementation head; dispatch task reviewer for spec and quality verdicts. Resolve material findings.
- [ ] Run the final build using isolated RUSTUP_HOME/CARGO_HOME from ../../work and npm cache from ../../work/npm-cache. Generate an ad-hoc signed Apple Silicon .app; updater artifacts disabled.
- [ ] Inspect built bundle identity and launch behavior where host permissions allow. Do not represent a browser UI preview as native microphone/global hotkey verification.
- [ ] Package the app, source changes and Chinese instructions. Record what passed and what requires the user's microphone/accessibility grant and configured API keys.
- [ ] Perform final branch review and report the deliverable links with concise setup instructions.
