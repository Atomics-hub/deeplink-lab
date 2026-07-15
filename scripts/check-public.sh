#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PRIVATE_ROOT_PATTERN='/(Users|home)/[A-Za-z0-9._-]+'
TOKEN_PATTERN='gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|AKIA[0-9A-Z]{16}|-----BEGIN (RSA |EC |OPENSSH )?PRIVATE KEY-----'
EMAIL_PATTERN='[A-Za-z0-9._%+-]+@(gmail|outlook|yahoo|icloud)\.com'

cd "$ROOT"
if rg -n --hidden \
  --glob '!target/**' \
  --glob '!outputs/**' \
  --glob '!work/**' \
  --glob '!fixtures/**/build/**' \
  --glob '!scripts/check-public.sh' \
  "$PRIVATE_ROOT_PATTERN|$TOKEN_PATTERN|$EMAIL_PATTERN" .; then
  echo "public-boundary scan found a private path, credential pattern, or personal mailbox" >&2
  exit 1
fi

if rg -n \
  --glob '!fixtures/**/build/**' \
  '(Instagram|TikTok|Facebook|Gmail|WhatsApp).*(password|login|credential|automation)' \
  src fixtures examples deeplinklab.yml; then
  echo "consumer-account automation boundary was crossed" >&2
  exit 1
fi

echo "public-boundary scan passed"
