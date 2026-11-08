#!/usr/bin/env bash
# Runs the tests marked `#[ignore = "slow: ..."]`: the whole-simulation integration tests that
# are too long for every platform's push run. CI's slow-test job runs them on Linux, split over
# a few shards; run them here the same way.
#
#   tools/ci/slow-tests.sh                 # every marked test
#   tools/ci/slow-tests.sh 2 3             # shard 2 of 3
#   tools/ci/slow-tests.sh --list          # the marked tests and their shard, nothing run
#
# A mark is found from the source, not from a list: each `slow:` mark names the `fn` that
# follows it, and that test is run by its exact name with `--ignored`. Every file's run must
# report as many passed tests as the file has marks, so a mark that names nothing (a test moved
# into a module, a renamed function) fails here instead of going unrun. Other ignored tests
# (measurements, probes, the Manhattan extract) are not run.
#
# The cargo profile is `ci` (see the workspace Cargo.toml); SLOW_TESTS_PROFILE overrides it.
set -euo pipefail
cd "$(dirname "$0")/../.."

list=0
if [ "${1:-}" = "--list" ]; then list=1; shift; fi
shard=${1:-1}
count=${2:-1}
profile=${SLOW_TESTS_PROFILE:-ci}

# `v2xw-py`'s marked tests run in the python job, which has the interpreter they embed.
skip_crates=" v2xw-py "

# Measured seconds per test file in an unoptimised build, used only to balance the shards; a
# file not listed counts as 120.
weight() {
  case "$1" in
    scms_knobs_runs) echo 4164 ;; in_the_loop) echo 1787 ;; long_run) echo 1769 ;;
    info_status) echo 862 ;; departed_state_scms) echo 668 ;; ma_policy_family) echo 622 ;;
    energy_glosa) echo 490 ;; violations) echo 484 ;; departed_state) echo 465 ;;
    sensitivity_radio) echo 436 ;; ma_features) echo 431 ;; sensitivity_security) echo 418 ;;
    security_lifecycle) echo 398 ;; soak) echo 352 ;; vehicle_dynamics) echo 343 ;;
    alternating_mac) echo 296 ;; report_store) echo 293 ;; emergency_yield) echo 280 ;;
    longrun_budget) echo 272 ;; ma_policy) echo 266 ;; *) echo 120 ;;
  esac
}

# One line per marked file: "<weight> <crate> <stem> <fn> <fn> ...".
files=()
while IFS= read -r f; do
  crate=$(sed -n 's/^name = "\(.*\)"/\1/p' "$(dirname "$(dirname "$f")")/Cargo.toml" | head -1)
  case "$skip_crates" in *" $crate "*) continue ;; esac
  stem=$(basename "$f" .rs)
  fns=$(awk '/#\[ignore = "slow: /{want=1; next} want && match($0, /fn [A-Za-z0-9_]+/) {print substr($0, RSTART+3, RLENGTH-3); want=0}' "$f" | tr '\n' ' ')
  files+=("$(weight "$stem") $crate $stem $fns")
done < <(grep -rlE --include='*.rs' '#\[ignore = "slow: ' crates | sort)

if [ ${#files[@]} -eq 0 ]; then
  echo "error: no test is marked slow; the search is broken" >&2
  exit 1
fi
for line in "${files[@]}"; do
  case "$line" in */*) echo "error: a slow test outside crates/*/tests/*.rs: $line" >&2; exit 1 ;; esac
done

# Heaviest file first onto the lightest shard (deterministic: ties go to the lower shard).
loads=()
for ((s = 1; s <= count; s++)); do loads[s]=0; done
mine=()
while IFS= read -r line; do
  w=${line%% *}
  best=1
  for ((s = 2; s <= count; s++)); do [ "${loads[s]}" -lt "${loads[best]}" ] && best=$s; done
  loads[best]=$((loads[best] + w))
  if [ $list = 1 ]; then echo "shard $best: ${line#* }"; fi
  [ "$best" = "$shard" ] && mine+=("${line#* }")
done < <(printf '%s\n' "${files[@]}" | sort -k1,1nr -k3,3)
[ $list = 1 ] && exit 0
if [ ${#mine[@]} -eq 0 ]; then
  echo "error: shard $shard of $count has no slow tests" >&2
  exit 1
fi

failed=0
for line in "${mine[@]}"; do
  read -r crate stem fns <<<"$line"
  want=$(wc -w <<<"$fns" | tr -d ' ')
  echo "::group::$crate --test $stem ($want slow)"
  log=$(mktemp)
  # shellcheck disable=SC2086 # the test names are separate arguments
  if ! cargo test --profile "$profile" -p "$crate" --test "$stem" -- --ignored --exact $fns 2>&1 | tee "$log"; then
    failed=1
  fi
  got=$(sed -n 's/^test result: [a-zA-Z]*\. \([0-9]*\) passed.*/\1/p' "$log" | tail -1)
  rm -f "$log"
  echo "::endgroup::"
  if [ "${got:-0}" != "$want" ]; then
    echo "::error::$crate --test $stem: $want tests are marked slow, ${got:-0} passed"
    failed=1
  fi
done
exit $failed
