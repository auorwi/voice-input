# Security Policy

This repository maintains the personal macOS edition of Voice Input. For a vulnerability in this edition, use [GitHub private vulnerability reporting](https://github.com/auorwi/voice-input/security/advisories/new). Do not disclose exploits, API keys, or private dictation content in public issues.

Include the affected commit, macOS version, impact, and minimal reproduction steps with sensitive data removed. This personal project does not promise a fixed response time.

## Data handling

- Configure your own STT and LLM providers. Their data policies and charges apply to the audio and text sent to them.
- Provider secrets are stored in the macOS Keychain; local application data includes configuration and dictation history.
- Accessibility permission supports global shortcuts, focused-input validation, and keyboard output. Unknown or changed destinations fall back to a copyable result window.
- This edition uses a separate application identifier and disables upstream automatic updates and account onboarding.

Keep `.env` files, credentials, databases, recordings, and local application state out of commits. Report an accidental credential exposure by revoking the credential first.
