#!/bin/zsh
# Touch UI test on the shared simulators (make ios-uitest [TESTS=...] [DEVICE=iphone|ipad|both]):
# XCTest synthesises pans, pinches, rotations, taps and the toolbar buttons (Tests/iOSUITests).
# 1. build-for-testing at low priority ($SIM_BG, $SIM_JOBS), WITHOUT the simulator lock;
# 2. take the machine-wide simulator lock, then per device: boot headless, test-without-building
#    (no parallel clones), shut the simulator down by UDID.
# TESTS: comma/space separated "Class" or "Class/testMethod" (the target name is added), empty
# = the whole suite. Screenshots: build/ios-screens/<idiom>-*.png (written by the test runner);
# result bundles build/ios-uitest-<device>.xcresult. kb: platform/ios-simulator.
# Usage: ios-uitest.sh iphone,ipad "TouchControlsUITests/testThrottle,FirstUseUITests"
set -euo pipefail
root=${0:A:h:h}
SIM_ROOT=$root
source $root/scripts/sim-common.zsh
devices=${1:-both}
tests=${2:-}
derived=$root/.build/xcode-ios-test
mkdir -p $root/build/ios-screens

only=()
for t in ${(s: :)${tests//,/ }}; do
  only+=("-only-testing:Almondbread iOS UITests/$t")
done

jobs=()
[[ -n ${SIM_JOBS:-} ]] && jobs=(-jobs $SIM_JOBS)
echo "ios-uitest: build-for-testing${SIM_BG:+ (${SIM_BG})}"
${=SIM_BG:-} xcodebuild -project $root/Almondbread.xcodeproj -scheme "Almondbread iOS" \
  -destination "generic/platform=iOS Simulator" -derivedDataPath $derived $jobs ARCHS=arm64 -quiet build-for-testing

sim_lock ios-uitest
sim_report_others
fail=0
booted=""
trap '[[ -n $booted ]] && sim_shutdown $booted' EXIT
for kind in $(sim_kinds $devices); do
  name=${SIM_NAME[$kind]:-}
  if [[ -z $name ]]; then echo "ios-uitest: unknown device '$kind' (iphone|ipad|both)"; fail=1; continue; fi
  slug=${${name:l}// /-}
  udid=$(sim_udid $kind)
  booted=$udid
  sim_boot $udid
  result=$root/build/ios-uitest-$slug.xcresult
  rm -rf $result
  echo "ios-uitest: $name${tests:+ ($tests)}"
  if TEST_RUNNER_ALMONDBREAD_UITEST_SHOTS=$root/build/ios-screens xcodebuild -project $root/Almondbread.xcodeproj \
       -scheme "Almondbread iOS" -destination "platform=iOS Simulator,id=$udid" \
       -derivedDataPath $derived -resultBundlePath $result \
       -parallel-testing-enabled NO "${only[@]}" -quiet test-without-building; then
    echo "ios-uitest: $name OK"
  else
    echo "ios-uitest: $name FAILED (result $result)"
    fail=1
  fi
  sim_shutdown $udid
  booted=""
done
exit $fail
