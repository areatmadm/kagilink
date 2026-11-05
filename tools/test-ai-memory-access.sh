#!/bin/sh
# POSIX sh equivalent of tools/Test-AiMemoryAccess.ps1 (owner-approved, D018).
# Same checks, same exit codes. Must not inspect private files or print auth secrets.
# Requires only the GitHub CLI (gh). Exit 0 = access allowed, exit 1 = denied.

deny() {
    # Do not include external errors: they can contain private URLs or diagnostics.
    echo 'AI_MEMORY_ACCESS_DENIED: skip all private memory; continue with public source documentation.'
    exit 1
}

command -v gh >/dev/null 2>&1 || deny

identity=$(gh api user --jq .login 2>/dev/null) || deny
[ "$identity" = "areatmadm" ] || deny

# jq comparisons are exact and case-sensitive; "== true" accepts only the JSON boolean true.
ok=$(gh api repos/areatmadm/kagilink-ai-memory --jq \
    '.full_name == "areatmadm/kagilink-ai-memory" and .owner.login == "areatmadm" and .private == true and .permissions.pull == true' \
    2>/dev/null) || deny
[ "$ok" = "true" ] || deny

echo 'AI_MEMORY_ACCESS_ALLOWED: authenticated areatmadm, expected private repository verified.'
exit 0
