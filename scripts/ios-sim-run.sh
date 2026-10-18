#!/bin/zsh
# The app in the shared simulator for the user to try by hand (make ios-sim-run [DEVICE=iphone]):
# the user's command, so it opens Simulator.app. It holds the simulator lock while it runs (a
# test job would otherwise install over the app and shut the simulator down under the user);
# agents' simulator jobs wait meanwhile and name this as the holder. Runs in the foreground with
# the app's output; Ctrl-C (or quitting the app) stops it: the app ends, the simulator shuts
# down, the lock is released. SIM_RUN_NO_WINDOW=1: headless, no Simulator.app (checks).
# kb: platform/ios-simulator. Usage: ios-sim-run.sh ipad|iphone path/to/Almondbread.app
set -euo pipefail
root=${0:A:h:h}
SIM_ROOT=$root
source $root/scripts/sim-common.zsh
kind=${1:-ipad}
app=$2
bundle=ch.ractive.almondbread
[[ -n ${SIM_NAME[$kind]:-} ]] || { echo "ios-sim-run: DEVICE must be ipad or iphone"; exit 2 }
sim_lock "ios-sim-run (interactive, the user's)"
udid=$(sim_udid $kind)
simulator_was_running=0
pgrep -xq Simulator && simulator_was_running=1
child=""
cleanup() {
  trap - EXIT INT TERM
  [[ -n $child ]] && kill $child 2>/dev/null
  echo "\nios-sim-run: stopping (app, simulator ${SIM_NAME[$kind]})"
  xcrun simctl terminate $udid $bundle 2>/dev/null || true
  sim_shutdown $udid
  # Quit Simulator.app only if we started it and nothing else is booted.
  if (( ! simulator_was_running )) && [[ -z ${SIM_RUN_NO_WINDOW:-} ]] \
     && ! xcrun simctl list devices booted | grep -q '(Booted)'; then
    osascript -e 'quit app "Simulator"' 2>/dev/null || true
  fi
}
trap cleanup EXIT
trap 'exit 130' INT TERM
sim_boot $udid
if [[ -z ${SIM_RUN_NO_WINDOW:-} ]]; then
  open -a Simulator --args -CurrentDeviceUDID $udid
fi
xcrun simctl install $udid $app
echo "ios-sim-run: ${SIM_NAME[$kind]} ($udid). Ctrl-C here stops the app and the simulator and"
echo "releases the simulator lock (agents' simulator jobs wait while this runs)."
# The app's output until it quits or Ctrl-C. In the background + wait, so a TERM/INT trap runs
# at once (zsh defers traps while a foreground child runs).
xcrun simctl launch --console-pty --terminate-running-process $udid $bundle ${=SIM_RUN_ARGS:-} &
child=$!
wait $child || true
child=""
