#!/bin/sh
# Fails if tracked files contain personal identifiers that must never reach this PUBLIC repository
# (D028): Apple signing identities with a person's name, device UDIDs, and private-key blocks.
# Patterns only; no real value is listed here. Usage: sh tools/check-no-personal-ids.sh
set -eu

P_IDENTITY='(Apple (Development|Distribution)|Developer ID (Application|Installer)): [^"]+ \([A-Z0-9]{10}\)'
P_UDID='\b[0-9A-Fa-f]{8}-[0-9A-Fa-f]{16}\b'
P_KEY='-----BEGIN ([A-Z ]*)PRIVATE KEY-----'

# Self-test: each pattern must match an obviously fake sample, otherwise the scan is blind.
selftest() {
  printf '%s\n' "$2" | grep -Eq -e "$1" || { echo "self-test failed for a pattern: scan would be blind"; exit 2; }
}
selftest "$P_IDENTITY" 'Apple Development: Test Person (ABCDE12345)'
selftest "$P_UDID" '00001111-0123456789ABCDEF'
selftest "$P_KEY" '-----BEGIN PRIVATE KEY-----'

fail=0
check() {
  label="$1"; pattern="$2"
  # A real git error (exit code > 1) must fail the check, not read as "no hits".
  rc=0
  hits=$(git grep -nIE -e "$pattern" -- . ':(exclude)tools/check-no-personal-ids.sh' ':(exclude)docs/DECISIONS.md') || rc=$?
  if [ "$rc" -gt 1 ]; then echo "git grep failed ($label)"; exit 2; fi
  if [ -n "$hits" ]; then
    echo "FOUND ($label):"
    # file:line only, never the matched text, so CI logs do not repeat the value.
    echo "$hits" | cut -d: -f1,2
    fail=1
  fi
}
check "signing identity with a name" "$P_IDENTITY"
check "device UDID" "$P_UDID"
check "private key block" "$P_KEY"
if [ "$fail" -ne 0 ]; then
  echo "Personal identifiers or key material found in tracked files. Use required environment variables instead (D028)."
  exit 1
fi
echo "no personal identifiers found in tracked files"
