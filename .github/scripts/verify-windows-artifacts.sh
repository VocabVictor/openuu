#!/usr/bin/env bash
# Fails unless <dir> holds exactly the three release files (portable zip, MSI,
# SHA-256 list) and every sum in the list verifies. Shared by windows-build.yml
# (after packing) and release.yml (after downloading the artifact), so a missing
# or corrupt file fails the run instead of publishing a partial set.
set -euo pipefail
dir="${1:?usage: verify-windows-artifacts.sh <dir>}"
cd "$dir"
shopt -s nullglob
zip=(openuu-*-x86_64-portable.zip)
msi=(openuu-*-x86_64.msi)
sums=(openuu-*-SHA256SUMS.txt)
fail=0
[ "${#zip[@]}" -eq 1 ]  || { echo "::error::expected exactly one portable zip, found ${#zip[@]}"; fail=1; }
[ "${#msi[@]}" -eq 1 ]  || { echo "::error::expected exactly one msi, found ${#msi[@]}"; fail=1; }
[ "${#sums[@]}" -eq 1 ] || { echo "::error::expected exactly one SHA256SUMS file, found ${#sums[@]}"; fail=1; }
if [ "$fail" -ne 0 ]; then ls -l; exit 1; fi
grep -q -- "${zip[0]}" "${sums[0]}" || { echo "::error::${sums[0]} does not list ${zip[0]}"; exit 1; }
grep -q -- "${msi[0]}" "${sums[0]}" || { echo "::error::${sums[0]} does not list ${msi[0]}"; exit 1; }
sha256sum -c "${sums[0]}"
echo "artifacts verified: ${zip[0]}, ${msi[0]}, ${sums[0]}"
