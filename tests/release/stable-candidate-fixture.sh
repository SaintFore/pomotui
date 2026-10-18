#!/usr/bin/env bash
# Exercise stable generation/build without selecting a real production release.
set -Eeuo pipefail
out=${1:?output directory required}
fixture=$(mktemp -d)
trap 'rm -rf "$fixture"' EXIT
git clone --quiet --no-hardlinks . "$fixture/source"
python3 - "$fixture/source" <<'PY'
from pathlib import Path
import re
import sys
root = Path(sys.argv[1])
manifest = root / 'Cargo.toml'
manifest.write_text(re.sub(r'^version = ".*"$', 'version = "0.0.0"', manifest.read_text(), count=1, flags=re.M))
lock = root / 'Cargo.lock'
lock.write_text(re.sub(r'(name = "pomotui-[^"]+"\nversion = ")[^"]+"', r'\g<1>0.0.0"', lock.read_text()))
PY
printf '%s\n' 'Nonpublishing build fixture; never a selected release.' > "$fixture/source/RELEASE_NOTES.md"
git -C "$fixture/source" add RELEASE_NOTES.md
git -C "$fixture/source" -c user.name='Nonpublishing fixture' -c user.email='fixture@example.invalid' commit --quiet -am 'Nonpublishing stable preparation fixture only'
printf '%s\n' 'Nonpublishing build fixture; never a selected release.' > "$fixture/notes.md"
python3 packaging/release/candidate.py plan --repo "$fixture/source" --intent stable --notes "$fixture/notes.md" --output "$out"
