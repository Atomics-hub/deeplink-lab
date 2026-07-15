#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

DEEPLINKLAB_MANIFEST="$ROOT/AndroidManifest.xml" \
DEEPLINKLAB_BUILD="$ROOT/build" \
DEEPLINKLAB_APK_BASENAME="CandidateFixture.apk" \
DEEPLINKLAB_DNAME="CN=Candidate Fixture,OU=Integration Proof,O=Fixture,L=Local,ST=None,C=US" \
  "$ROOT/../android/build.sh"
