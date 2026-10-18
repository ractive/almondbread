# Shared by the simulator scripts (ios-selftest.sh, ios-uitest.sh, ios-sim-run.sh); source it.
# kb: platform/ios-simulator "Low-load rules".
#
# - sim_lock LABEL: the machine-wide simulator lock. One simulator job at a time across all
#   worktrees and agents; a second one waits and says who holds it. An fcntl lock held by this
#   shell (zsystem flock): it is released when the script exits or dies, so a crashed holder
#   never leaves a stale lock. Lock file ~/Library/Caches/almondbread/sim.lock, holder in
#   sim.lock.holder (informational only).
# - sim_udid iphone|ipad: the ONE shared simulator of that kind (fixed names below), created on
#   demand and reused by every worktree. Never make per-worktree or per-agent simulators.
# - sim_boot UDID / sim_shutdown UDID: boot headless (no Simulator.app) and wait until it is
#   usable; shut down by UDID afterwards (never `simctl shutdown all`: that kills other people's
#   simulators).
zmodload zsh/system

SIM_CACHE=$HOME/Library/Caches/almondbread
SIM_LOCK=$SIM_CACHE/sim.lock
SIM_HOLDER=$SIM_CACHE/sim.lock.holder
SIM_RUNTIME=${SIM_RUNTIME:-}
# Shared pair: names and device types. "AB iPhone 17e" / "AB iPad mini" are the first pair
# (2026-10-15), already through their slow first boot.
typeset -gA SIM_NAME SIM_TYPE
SIM_NAME=(iphone "AB iPhone 17e" ipad "AB iPad mini")
SIM_TYPE=(iphone com.apple.CoreSimulator.SimDeviceType.iPhone-17e
          ipad com.apple.CoreSimulator.SimDeviceType.iPad-mini-A17-Pro)

# sim_lock LABEL [WAIT_SECONDS]: blocks until the lock is ours (default wait 90 min, then fails).
sim_lock() {
  local label=$1 wait=${2:-${SIM_LOCK_WAIT:-5400}} waited=0
  mkdir -p $SIM_CACHE
  [[ -e $SIM_LOCK ]] || : >> $SIM_LOCK
  if ! zsystem flock -t 0 -f SIM_LOCK_FD $SIM_LOCK 2>/dev/null; then
    local holder="(unknown)"
    [[ -r $SIM_HOLDER ]] && holder=$(<$SIM_HOLDER)
    echo "$label: waiting for the simulator lock held by $holder"
    while ! zsystem flock -t 0 -f SIM_LOCK_FD $SIM_LOCK 2>/dev/null; do
      if (( waited >= wait )); then
        echo "$label: gave up after ${waited} s waiting for the simulator lock ($SIM_LOCK)" >&2
        return 1
      fi
      sleep 5; (( waited += 5 ))
      if (( waited % 300 == 0 )); then
        [[ -r $SIM_HOLDER ]] && holder=$(<$SIM_HOLDER)
        echo "$label: still waiting (${waited} s) for the simulator lock held by $holder"
      fi
    done
    echo "$label: got the simulator lock after ${waited} s"
  fi
  # Never redirect into $SIM_LOCK itself while holding it: that would drop the fcntl lock.
  print -r -- "$label in ${SIM_ROOT:-$PWD}, pid $$, since $(date '+%H:%M:%S')" > $SIM_HOLDER
}

# The simulators booted right now that are not ours (another job under the old scripts, or the
# user's own): only reported, they are not ours to stop.
sim_report_others() {
  local others
  others=$(xcrun simctl list devices booted | grep -E '\(Booted\)' | grep -vF -e "${SIM_NAME[iphone]} (" -e "${SIM_NAME[ipad]} (" || true)
  [[ -n $others ]] && print -r -- "note: other simulators are booted (not ours, left alone):"$'\n'"$others"
  return 0
}

# sim_udid iphone|ipad: UDID of the shared simulator, created if missing.
sim_udid() {
  local kind=$1 name=${SIM_NAME[$1]} udid runtime
  [[ -n $name ]] || { echo "unknown device '$kind' (iphone|ipad)" >&2; return 1 }
  udid=$(xcrun simctl list devices -j | /usr/bin/python3 -c '
import json, sys
n = sys.argv[1]
for rt, ds in json.load(sys.stdin)["devices"].items():
    for d in ds:
        if d["name"] == n and d.get("isAvailable", True):
            print(d["udid"]); sys.exit()' "$name")
  if [[ -z $udid ]]; then
    runtime=$SIM_RUNTIME
    [[ -n $runtime ]] || runtime=$(xcrun simctl list runtimes -j | /usr/bin/python3 -c '
import json, sys
rs = [r for r in json.load(sys.stdin)["runtimes"] if r["platform"] == "iOS" and r["isAvailable"]]
print(max(rs, key=lambda r: [int(x) for x in r["version"].split(".")])["identifier"] if rs else "")')
    [[ -n $runtime ]] || { echo "no iOS simulator runtime installed (xcodebuild -downloadPlatform iOS)" >&2; return 1 }
    udid=$(xcrun simctl create "$name" ${SIM_TYPE[$kind]} $runtime)
    echo "created the shared simulator '$name' ($udid); its first boot can take minutes" >&2
  fi
  print -r -- $udid
}

sim_boot() {
  xcrun simctl boot $1 2>/dev/null || true
  xcrun simctl bootstatus $1 -b >/dev/null
}

sim_shutdown() {
  xcrun simctl shutdown $1 2>/dev/null || true
}

# DEVICE=iphone|ipad|both (or a comma list) -> kinds, one per line.
sim_kinds() {
  local d=${1:-both}
  [[ $d == both || $d == all ]] && d=iphone,ipad
  print -l -- ${(s:,:)d}
}
