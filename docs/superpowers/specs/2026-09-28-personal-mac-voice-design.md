# Personal Mac voice input — approved design

The user approved development on 2026-09-28 after choosing personal use, macOS only, and cloud processing with their own provider credentials. Base: OpenTypeless v1.1.60, commit 842f278191c39e22fa21efec347c750245c91170 (MIT).

## User experience

- First shortcut key-down starts recording. Release does not stop it. A second distinct key-down ends recording and produces cleaned text. Ignore key-repeat and busy-state presses; Escape cancels active recording/processing without output.
- A non-activating recording capsule shows recording and processing state without stealing the target application's input focus.
- If a writable input has focus and the destination remains trustworthy, insert the completed text there. Mouse pointer position is irrelevant.
- If no writable input has focus, the target has changed, focus cannot be determined, or output fails, show a dedicated result window containing the complete available text and a prominent Copy button. Do not silently lose text or claim insertion merely because synthetic keystrokes were dispatched.
- Copy success gives visible feedback; clipboard failure leaves text visible and selectable. The result window never disappears on a timer; explicit Close/Escape closes it. Closing it does not delete the saved history entry.
- A result arriving before its renderer is ready remains retrievable. Consecutive recordings must not erase an unconsumed result: keep multiple pending results navigable, or bring existing result forward and require dismissal before recording again.
- Each result belongs to one recording; an old window's actions cannot dismiss or overwrite a newer result. Starting a new recording must not accidentally record the app's own result window as a text destination.
- Preserve local history, personal dictionary, Chinese/mixed-language input and existing configurable STT/LLM providers. Default to Chinese UI, light faithful cleanup, and BYOK onboarding. Do not request or embed secrets in source or logs.

## Architecture and boundaries

Reuse the Rust/Tauri capture, provider, history, hotkey and non-activating capsule infrastructure. Add a small macOS accessibility adapter for focused editable-element observation, with an explicit unknown state. Check output eligibility immediately before output and account for input target changes within one app. Use native Accessibility APIs where practical, without reading unrelated document contents or screens. Unsupported/custom controls may conservatively fall back to the result window.

Add a backend-owned result payload/store and a dedicated Tauri result window with a React view and Copy/Close actions. Gate every ordinary dictation output path, including unpolished output and LLM-failure raw-transcript fallback. On macOS keep final output atomic by disabling streaming insertion for this personal build; streaming model responses may still occur internally.

Keep credentials configurable using existing provider UI and Keychain integration. API configuration and real cloud transcription must be tested by the user after entering keys; no real provider calls are authorized with fabricated credentials.

The personal build uses its own application name (Voice Input), bundle identifier (com.local.voiceinput.personal), credential namespace where upstream uses fixed names, and disabled upstream update/deep-link ownership, to avoid overwriting or impersonating an installed upstream application. Keep MIT notices and acknowledge the upstream project. Do not publish, push or install into /Applications.

## Validation

Behavior tests cover toggle start/release/stop and repeated/busy presses; writable vs non-writable/unknown/read-only/secure targets and changed targets; output failure preserving full text; result readiness, copy feedback/failure, explicit dismissal and multiple sessions. Run relevant Rust tests, frontend tests, type/build/lint checks, and build an Apple Silicon .app if toolchain permits. Inspect the actual built application or an honest UI preview where permissions/provider keys prevent end-to-end audio validation. Document unverified runtime behavior plainly.

## Deliverables

Modified source, local .app/zip if build succeeds, and a concise Chinese setup/usage guide explaining microphone/accessibility permissions, provider configuration, shortcut behavior and result fallback. Deliverables stay in this chat's outputs directory. Build tools and transient data stay in work.
