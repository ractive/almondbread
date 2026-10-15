#!/bin/zsh
# iOS self-test on simulators (make ios-selftest): installs the simulator build, launches it
# headless (no Simulator.app window) with the self-test, waits for "renderer OK", copies the
# offscreen PNG to build/ios-selftest-<device>.png and takes a screenshot of the device to
# build/ios-screens/<device>-selftest.png. Usage: ios-selftest.sh "Name A,Name B" path/to/Almondbread.app
set -euo pipefail
devices=$1
app=$2
bundle=ch.ractive.almondbread.ios
root=${0:A:h:h}
mkdir -p $root/build/ios-screens
fail=0
for name in ${(s:,:)devices}; do
  slug=${${name:l}// /-}
  udid=$(xcrun simctl list devices available -j | /usr/bin/python3 -c "import json,sys; n=sys.argv[1]; print(next((d['udid'] for r in json.load(sys.stdin)['devices'].values() for d in r if d['name']==n), ''))" "$name")
  if [[ -z $udid ]]; then echo "ios-selftest: no simulator named '$name'"; fail=1; continue; fi
  xcrun simctl boot $udid 2>/dev/null || true
  xcrun simctl bootstatus $udid -b >/dev/null
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
done
exit $fail
