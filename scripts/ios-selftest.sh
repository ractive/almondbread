#!/bin/zsh
# iOS self-test on the shared simulators (make ios-selftest [DEVICE=iphone|ipad|both]): takes the
# machine-wide simulator lock, then per device: boots it headless (no Simulator.app window),
# installs the simulator build, launches it with the self-test, waits for "renderer OK", copies
# the offscreen PNG to build/ios-selftest-<device>.png, takes a screenshot of a normal launch to
# build/ios-screens/<device>-launch.png and shuts the simulator down again (one booted at a time).
# kb: platform/ios-simulator. Usage: ios-selftest.sh iphone,ipad path/to/Almondbread.app
set -euo pipefail
root=${0:A:h:h}
SIM_ROOT=$root
source $root/scripts/sim-common.zsh
devices=$1
app=$2
bundle=ch.ractive.almondbread
mkdir -p $root/build/ios-screens
sim_lock ios-selftest
sim_report_others
fail=0
booted=""
trap '[[ -n $booted ]] && sim_shutdown $booted' EXIT
for kind in $(sim_kinds $devices); do
  name=${SIM_NAME[$kind]:-}
  if [[ -z $name ]]; then echo "ios-selftest: unknown device '$kind' (iphone|ipad|both)"; fail=1; continue; fi
  slug=${${name:l}// /-}
  udid=$(sim_udid $kind)
  booted=$udid
  sim_boot $udid
  xcrun simctl install $udid $app
  log=$root/build/ios-selftest-$slug.log
  # --console-pty: the app's stdout; it quits by itself (exit status 1/2 on failure).
  if ! SIMCTL_CHILD_ALMONDBREAD_SELFTEST=1 ${=commands[timeout]:+timeout 240} xcrun simctl launch --console-pty --terminate-running-process \
       $udid $bundle --selftest-png=selftest.png >$log 2>&1; then
    true
  fi
  cat $log | grep -E "renderer|selftest" || true
  if grep -q "renderer OK" $log; then
    data=$(xcrun simctl get_app_container $udid $bundle data)
    cp $data/Documents/selftest.png $root/build/ios-selftest-$slug.png
    echo "ios-selftest: $name OK, PNG build/ios-selftest-$slug.png"
  else
    echo "ios-selftest: $name FAILED (log $log)"
    fail=1
  fi
  # A normal launch for a screenshot of the UI (HUD, toolbar buttons).
  xcrun simctl launch --terminate-running-process $udid $bundle >/dev/null
  sleep 6
  xcrun simctl io $udid screenshot $root/build/ios-screens/$slug-launch.png >/dev/null 2>&1
  xcrun simctl terminate $udid $bundle 2>/dev/null || true
  sim_shutdown $udid
  booted=""
done
exit $fail
