#!/usr/bin/env bash
#
# SolID Protocol -- pre-commit secrets gate.
#
# Blocks commits that stage (a) known-plaintext E2E state filenames or
# (b) any file whose staged diff introduces a private-key JSON field.
#
# Closes SOLID-SEC-020. Root cause: scripts/issue.ts:118-125 persists
# issuer and holder secrets to scripts/e2e_state.json, which is not in
# .gitignore in older checkouts. This hook is defense-in-depth; the
# .gitignore entries are the first line.
#
# Install:
#   ln -sf ../../scripts/hooks/pre-commit-no-secrets.sh \
#          .git/hooks/pre-commit
#   chmod +x scripts/hooks/pre-commit-no-secrets.sh
#
# Bypass (discouraged; leave an audit trail):
#   git commit --no-verify ...
#
# Manual run:
#   bash scripts/hooks/pre-commit-no-secrets.sh

set -euo pipefail

FORBIDDEN_FILE_REGEX='(^|/)(e2e_state(\.[^/]*)?\.json|\.secrets/)'

# Field names that, in staged content, typically signal a plaintext keypair
# check-in. Any match aborts the commit.
FORBIDDEN_FIELDS=(
    'privateKey'
    'secretKey'
    'holderPrivateKey'
    'issuerPrivateKey'
    'masterPrivateKey'
    'bjjPrivateKey'
)

staged_files=$(git diff --cached --name-only --diff-filter=ACM || true)

if [ -z "${staged_files:-}" ]; then
    exit 0
fi

fail=0

# Check filenames.
while IFS= read -r f; do
    [ -z "$f" ] && continue
    if echo "$f" | grep -qE "$FORBIDDEN_FILE_REGEX"; then
        echo "pre-commit-no-secrets: forbidden file staged: $f" >&2
        echo "    Pattern: $FORBIDDEN_FILE_REGEX" >&2
        echo "    Add to .gitignore or move under scripts/.secrets/." >&2
        fail=1
    fi
done <<< "$staged_files"

# Check staged content for forbidden JSON fields.
while IFS= read -r f; do
    [ -z "$f" ] && continue
    [ -f "$f" ] || continue
    # Only scan textual diffs to avoid false positives on binaries.
    if file "$f" 2>/dev/null | grep -q 'text'; then
        diff_text=$(git diff --cached -- "$f" || true)
        for field in "${FORBIDDEN_FIELDS[@]}"; do
            if echo "$diff_text" | grep -qE "\"${field}\"[[:space:]]*:"; then
                echo "pre-commit-no-secrets: forbidden field '${field}' in staged diff of $f" >&2
                echo "    If this is legitimate test-fixture data, move the file" >&2
                echo "    under a .gitignore-ed path (scripts/.secrets/)." >&2
                fail=1
            fi
        done
    fi
done <<< "$staged_files"

if [ "$fail" -ne 0 ]; then
    exit 1
fi

exit 0
