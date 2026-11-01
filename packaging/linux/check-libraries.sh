#!/bin/sh
# Author: David M. Anderson
# Built with AI assistance (Claude, Anthropic)
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
target_dir=$(cd "$root" && cargo metadata --format-version 1 --no-deps |
    sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')
binary="${1:-${target_dir}/release/vier-desktop}"
[ -x "$binary" ] || { echo "check-libraries.sh: no executable at $binary; run cargo build --release first" >&2; exit 1; }

stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT INT TERM
: > "${stage}/libs"

observe() {
    label=$1
    shift
    "$@" "$binary" >"${stage}/${label}.log" 2>&1 &
    pid=$!
    last=0
    same=0
    i=0
    while [ "$i" -lt 100 ]; do
        kill -0 "$pid" 2>/dev/null || break
        now=$(awk '$6 ~ /\.so/ {print $6}' "/proc/$pid/maps" 2>/dev/null | sort -u | wc -l)
        if [ "$now" -eq "$last" ] && [ "$now" -gt 0 ]; then
            same=$((same + 1))
            [ "$same" -ge 5 ] && break
        else
            same=0
        fi
        last=$now
        i=$((i + 1))
        sleep 0.2
    done
    kill -0 "$pid" 2>/dev/null || { echo "check-libraries.sh: the window exited under $label" >&2; cat "${stage}/${label}.log" >&2; exit 1; }
    awk '$6 ~ /^\// && $6 ~ /\.so/ {print $6}' "/proc/$pid/maps" | sort -u > "${stage}/${label}.libs"
    kill "$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null || true
    count=$(wc -l < "${stage}/${label}.libs")
    [ "$count" -ge 20 ] || { echo "check-libraries.sh: only $count objects mapped under $label; the sample ended early" >&2; exit 1; }
    cat "${stage}/${label}.libs" >> "${stage}/libs"
    echo "  $label: $count objects mapped"
}

echo "running:"
if [ -n "${WAYLAND_DISPLAY:-}" ]; then observe wayland env WINIT_UNIX_BACKEND=wayland; else echo "  wayland: SKIPPED"; fi
if [ -n "${DISPLAY:-}" ]; then observe x11 env -u WAYLAND_DISPLAY WINIT_UNIX_BACKEND=x11; else echo "  x11: SKIPPED"; fi
sort -u "${stage}/libs" -o "${stage}/libs"

python3 - "${root}/Cargo.toml" "${stage}/libs" <<'PY'
import collections, subprocess, sys, tomllib

manifest, libs_file = sys.argv[1], sys.argv[2]
depends = tomllib.load(open(manifest, 'rb'))['package']['metadata']['deb']['depends']
declared = ['libc6', 'libgcc-s1'] + [
    alt.split('|')[0].strip().split()[0]
    for alt in depends.split(',') if alt.strip() != '$auto'
]

seen, queue = set(), collections.deque(declared)
while queue:
    pkg = queue.popleft()
    if pkg in seen:
        continue
    seen.add(pkg)
    out = subprocess.run(
        ['apt-cache', 'depends', '--no-recommends', '--no-suggests', '--no-conflicts',
         '--no-breaks', '--no-replaces', '--no-enhances', pkg],
        capture_output=True, text=True).stdout
    for line in out.splitlines():
        line = line.strip()
        if line.startswith(('Depends:', 'PreDepends:')):
            dep = line.split(':', 1)[1].strip().lstrip('<').rstrip('>')
            if dep and not dep.startswith('|'):
                queue.append(dep)

optional = {'mesa-vulkan-drivers'}
libs = [line.strip() for line in open(libs_file) if line.strip()]
missing = {}
for lib in libs:
    owner = subprocess.run(['dpkg', '-S', lib], capture_output=True, text=True).stdout
    owner = owner.split(':', 1)[0].strip()
    if owner and owner not in seen and owner not in optional:
        missing.setdefault(owner, []).append(lib.rsplit('/', 1)[-1])

print(f"\n{len(libs)} objects opened, {len(seen)} packages reachable from depends")
if not missing:
    print("every library the window opened is reachable from depends")
    sys.exit(0)
print("\nnot reachable from depends:")
for owner, names in sorted(missing.items()):
    print(f"  {owner}: {', '.join(names)}")
sys.exit(1)
PY
