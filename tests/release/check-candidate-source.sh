#!/usr/bin/env bash
# Run applicable checks against the exact retained source, not its parent checkout.
set -Eeuo pipefail
candidate=$(realpath "${1:?candidate directory}")
output=${2:?diagnostic directory}
mkdir -p "$output"
output=$(realpath "$output")
source_dir=$(mktemp -d)
trap 'rm -rf "$source_dir"' EXIT
source_commit=$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["source_commit"])' "$candidate/candidate.json")
git clone --quiet "$candidate/source.bundle" "$source_dir"
git -C "$source_dir" checkout --quiet --detach "$source_commit"
cd "$source_dir"
[[ "$(git rev-parse HEAD)" == "$source_commit" ]]
exec > >(tee "$output/source-checks.log") 2>&1
rustup toolchain install
cargo fmt --all --check
sh packaging/tests/aur-install.sh
python3 -m unittest discover -s tests/release -p '*_test.py'
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
cargo build --workspace
cargo build --release --workspace
tests/e2e.sh
tests/reservations-e2e.sh
sh tests/reservation-failure-e2e.sh
python3 - "$candidate" "$source_commit" <<'PY'
import json,pathlib,sys
(pathlib.Path(sys.argv[1])/'checks.json').write_text(json.dumps({'passed':True,'source_commit':sys.argv[2]})+'\n')
PY
