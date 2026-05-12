This repository is part of zz-drop.

Follow `AGENTS.md` as the source of truth.

When generating code:

- preserve documented CLI/API behavior
- do not invent new features
- do not duplicate logic across repositories
- never log secrets (passphrases, provider credentials, tokens, decrypted profile data, Authorization headers, session tokens)
- update docs/tests when behavior changes
- avoid scope creep: no sync, no mount, no public share-link flow, no generic cloud file manager
