#!/bin/sh
# Fails if the shared library exports anything but the functions declared in kagilink.h (D016).
# Usage: tools/check-ffi-exports.sh path/to/libareatm_kagilink_ffi.so
set -eu
lib="$1"
hdr="core/kagilink-ffi/include/kagilink.h"
want=$(grep -o 'kl_[a-z_0-9]*(' "$hdr" | tr -d '(' | sort -u)
have=$(nm -D --defined-only "$lib" | awk '{print $3}' | grep '^kl_' | sort -u)
if [ "$want" != "$have" ]; then
  echo "export mismatch"
  echo "--- header"; echo "$want"
  echo "--- library"; echo "$have"
  exit 1
fi
extra=$(nm -D --defined-only "$lib" | awk '{print $3}' | grep -v '^kl_' | grep -v '^_' || true)
if [ -n "$extra" ]; then
  echo "non-kl_ exports present:"; echo "$extra"; exit 1
fi
echo "exports match the header: $(echo "$want" | wc -l) functions"
