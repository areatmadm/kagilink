# AI conversation memory access

Read root AGENTS.md before any private-memory access. Public source contains no
conversation transcripts or original conversation prompts. Public requirements,
architecture, decisions and implementation status suffice for contribution.

Private memory lives in a separate Git repository, `areatmadm/pwreceiver-ai-memory`,
cloned at `shared/ai/` inside this project. The source .gitignore excludes that
entire directory. It is not a submodule and normal source clones require no access
to the private repository. No memory may be stored in USER/HOME folders.

## Owner-only agent workflow

```powershell
pwsh -NoProfile -File tools/Test-AiMemoryAccess.ps1
```

Exit 0: active GitHub API identity is exactly areatmadm; the exact expected private
repository and readable access were verified. Only then may an agent inspect the
private directory, its README, conversation index and latest record. Recheck at the
start of each task or after an account change. Do not use git config user.name as
an identity check. Never request that users paste authentication tokens into chat.

Exit 1: skip ALL private-memory reads/searches/writes and continue source work
using public docs. This includes API/network/auth failures, wrong user, public or
renamed repository, missing access and malformed responses. Do not switch accounts
automatically. If the account already has valid authorization, initialize with:

```powershell
pwsh -NoProfile -File tools/Initialize-AiMemory.ps1
```

The helper rechecks access and either clones into the ignored path or verifies an
existing nested repository. It does not read conversations or automatically pull
over local edits. Once authorized, maintain/push memory with `git -C shared/ai ...`
separately from source commits. Keep the private repo private with no collaborators.
Do not attach its content to public issues, logs, artifacts or workflow output.

## Actual security boundary

GitHub authentication/private visibility controls remote access. This script and
AGENTS.md control cooperative AI behavior, not OS file permissions. A local copy
is readable by any process/account granted filesystem access, regardless of its
active GitHub identity. Use OS access controls/device protection for local secrecy;
do not promise that a text instruction or .gitignore makes local files unreadable.

Prior source history containing memory must remain private. A clean public-source
repository must have no private-memory paths or original prompts in any published
commit, tag, branch or artifact. Keep the old repository private as an archive.
