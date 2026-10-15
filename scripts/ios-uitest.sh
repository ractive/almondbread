#!/bin/zsh
# Touch UI test on simulators (make ios-uitest): XCTest synthesises pans, pinches, rotations,
# taps and the toolbar buttons (Tests/iOSUITests). Screenshots: build/ios-screens/<idiom>-*.png
# (written by the test runner) and the .xcresult under build/ios-uitest-<device>.xcresult.
# Usage: ios-uitest.sh "Name A,Name B"
set -euo pipefail
devices=$1
root=${0:A:h:h}
mkdir -p $root/build/ios-screens
fail=0
for name in ${(s:,:)devices}; do
  slug=${${name:l}// /-}
  result=$root/build/ios-uitest-$slug.xcresult
  rm -rf $result
  if TEST_RUNNER_ALMONDBREAD_UITEST_SHOTS=$root/build/ios-screens xcodebuild -project $root/Almondbread.xcodeproj \
       -scheme "Almondbread iOS" -destination "platform=iOS Simulator,name=$name" \
       -derivedDataPath $root/.build/xcode-ios-test -resultBundlePath $result -quiet test; then
    echo "ios-uitest: $name OK"
  else
    echo "ios-uitest: $name FAILED (result $result)"
    fail=1
  fi
done
exit $fail
