#!/bin/zsh
# iPhone/iPad device builds (kb: platform/ios-device). Never touches the simulator build
# (make ios-sim stays unsigned) or the macOS signing.
#
#   ios-device.sh check     compile for iphoneos (arm64), unsigned: no device or account needed
#   ios-device.sh run       signed build (automatic signing, team IOS_TEAM), install, launch
#   ios-device.sh selftest  the same, then launch with the self-test, wait for "renderer OK",
#                           copy its PNG back to build/ios-device-selftest.png
#   ios-device.sh doctor    what is connected, Developer Mode, the Xcode account, the identity
#
# Environment: IOS_TEAM (default 9KNMHVRSTW), IOS_CONFIG (Release), IOS_DEVICE (name, UDID or
# identifier; default: the one connected iPhone/iPad).
set -uo pipefail
cmd=${1:-run}
root=${0:A:h:h}
team=${IOS_TEAM:-9KNMHVRSTW}
config=${IOS_CONFIG:-Release}
bundle=ch.ractive.almondbread
derived=$root/.build/xcode-ios-device
tmp=$(mktemp -d -t almondbread-device)
trap 'rm -rf $tmp' EXIT

die() { print -u2 "ios-device: $*"; exit 1; }
say() { print "ios-device: $*"; }

# The connected device: sets udid, ident, dname, dmode, paired, dtype. Clear errors otherwise.
find_device() {
  xcrun devicectl list devices --json-output $tmp/devices.json -q >/dev/null 2>&1 \
    || die "xcrun devicectl failed (Xcode 15 or later needed: xcode-select -p is $(xcode-select -p))"
  /usr/bin/python3 -I - $tmp/devices.json "${IOS_DEVICE:-}" > $tmp/device.txt <<'PY'
import json, sys
data = json.load(open(sys.argv[1]))
want = sys.argv[2]
rows = []
for d in data.get("result", {}).get("devices", []):
    hw, dp, cp = d.get("hardwareProperties", {}), d.get("deviceProperties", {}), d.get("connectionProperties", {})
    if hw.get("platform") not in ("iOS", "iPadOS") or hw.get("reality", "physical") != "physical":
        continue
    connected = cp.get("tunnelState") == "connected" or cp.get("transportType") in ("wired", "localNetwork")
    ids = {d.get("identifier", ""), hw.get("udid", ""), dp.get("name", ""), hw.get("serialNumber", "")}
    if want and want not in ids:
        continue
    rows.append((connected, hw.get("udid", ""), d.get("identifier", ""), dp.get("name", "?"),
                 dp.get("developerModeStatus", "unknown"), cp.get("pairingState", "unknown"),
                 hw.get("marketingName") or hw.get("productType") or hw.get("deviceType", "?"),
                 dp.get("osVersionNumber", "?")))
rows.sort(key=lambda r: not r[0])
live = [r for r in rows if r[0]]
if not live:
    print("none\t" + ";".join(f"{r[3]} ({r[6]}, {r[5]}, not connected)" for r in rows))
elif len(live) > 1 and not want:
    print("many\t" + ";".join(f"{r[3]} ({r[1]})" for r in live))
else:
    r = live[0]
    print("\t".join(["one", r[1], r[2], r[3], r[4], r[5], r[6], r[7]]))
PY
  local -a f
  f=("${(@ps:\t:)$(<$tmp/device.txt)}")
  case $f[1] in
    none)
      print -u2 "ios-device: no iPhone/iPad connected."
      [[ -n ${f[2]:-} ]] && print -u2 "  Known but not connected: ${f[2]}"
      print -u2 "  Connect it with a USB cable, unlock it and tap Trust (enter the passcode)."
      print -u2 "  'xcrun devicectl list devices' must show it as connected."
      exit 1;;
    many) die "several devices connected: ${f[2]}; pick one with IOS_DEVICE=<name or UDID>";;
  esac
  udid=$f[2]; ident=$f[3]; dname=$f[4]; dmode=$f[5]; paired=$f[6]; dtype=$f[7]
  say "device: $dname ($dtype, iOS $f[8], UDID $udid, pairing $paired, Developer Mode $dmode)"
  if [[ $paired != paired && $paired != unknown ]]; then
    die "the device is not paired: unlock it, tap Trust This Computer and enter the passcode, then run again"
  fi
  if [[ $dmode == disabled ]]; then
    print -u2 "ios-device: Developer Mode is off on $dname."
    print -u2 "  On the device: Settings > Privacy & Security > Developer Mode > On, restart, then confirm"
    print -u2 "  'Turn On' after the restart. If the switch is missing, open Xcode > Window > Devices and"
    print -u2 "  Simulators once with the device connected (that reveals it), or run this once: the"
    print -u2 "  first install attempt also makes it appear."
    exit 1
  fi
}

# An Xcode account (needed once for automatic signing) or an Apple Development identity.
has_identity() { security find-identity -v -p codesigning 2>/dev/null | grep -q "Apple Development"; }
has_account() {
  local d
  for key in DVTDeveloperAccountManagerAppleIDLists IDEProvisioningTeamByIdentifier IDEProvisioningTeams; do
    d=$(defaults read com.apple.dt.Xcode $key 2>/dev/null) && [[ -n $d ]] && return 0
  done
  return 1
}
check_account() {
  if has_account; then
    has_identity || say "no Apple Development certificate yet: xcodebuild creates one for team $team"
    return 0
  fi
  if has_identity; then
    say "no Xcode account found, but an Apple Development certificate is in the keychain: trying"
    return 0
  fi
  print -u2 "ios-device: Xcode is not signed in to a developer account (needed once for signing)."
  print -u2 "  Xcode > Settings (Cmd-,) > Accounts > + > Apple ID: sign in with the Apple ID of team $team."
  print -u2 "  Then run this again; xcodebuild creates the certificate and the profile by itself."
  exit 1
}

# Known xcodebuild failures in plain words.
explain_build_failure() {
  local log=$1
  if grep -qE "No Accounts|No account for team|not signed in|requires a development team" $log; then
    print -u2 "  -> Xcode has no account for team $team: Xcode > Settings > Accounts > + (Apple ID), then retry."
  elif grep -qE "No signing certificate|doesn't include signing certificate|Signing certificate is invalid" $log; then
    print -u2 "  -> No Apple Development certificate for team $team. Xcode > Settings > Accounts > the team >"
    print -u2 "     Manage Certificates > + Apple Development, or retry (-allowProvisioningUpdates makes one)."
  elif grep -qE "Developer Mode" $log; then
    print -u2 "  -> Turn on Developer Mode on the device (Settings > Privacy & Security > Developer Mode)."
  elif grep -qE "is not registered|has not been added|Unable to add device|device limit" $log; then
    print -u2 "  -> The device could not be registered with team $team (developer.apple.com > Devices)."
  elif grep -qE "not available for|cannot be used|Unable to find a destination" $log; then
    print -u2 "  -> Xcode cannot use the device yet: keep it unlocked and connected; Xcode may still be"
    print -u2 "     copying its debug symbols (Window > Devices and Simulators shows the progress)."
  elif grep -qE "bundle identifier .* (is not available|cannot be registered)" $log; then
    print -u2 "  -> The bundle ID $bundle belongs to another team; build with PRODUCT_BUNDLE_IDENTIFIER changed."
  fi
}

project() {
  (cd $root && xcodegen generate --quiet) || die "xcodegen failed"
  make -C $root rust >/dev/null || die "make rust failed"
}

build_signed() {
  local log=$root/build/ios-device-build.log
  mkdir -p $root/build
  say "building $config for $dname, team $team (automatic signing; log build/ios-device-build.log)"
  # Command-line settings override the target's simulator settings (signing off) for this build
  # only; -allowProvisioningUpdates creates the certificate, App ID and profile, and
  # -allowProvisioningDeviceRegistration adds the device to the team.
  if ! xcodebuild -project $root/Almondbread.xcodeproj -scheme "Almondbread iOS" -configuration $config \
      -destination "id=$udid" -derivedDataPath $derived \
      -allowProvisioningUpdates -allowProvisioningDeviceRegistration \
      DEVELOPMENT_TEAM=$team CODE_SIGN_STYLE=Automatic CODE_SIGN_IDENTITY="Apple Development" \
      CODE_SIGNING_ALLOWED=YES CODE_SIGNING_REQUIRED=YES \
      build >$log 2>&1; then
    grep -E "error:|warning: .*(sign|provision)" $log | sort -u | head -20 >&2
    print -u2 "ios-device: the build failed (full log: build/ios-device-build.log)"
    explain_build_failure $log
    exit 1
  fi
  app=$derived/Build/Products/$config-iphoneos/Almondbread.app
  [[ -d $app ]] || die "built, but no app at $app"
  codesign -dv $app 2>&1 | grep -E "^(Identifier|TeamIdentifier)=" | sed 's/^/  /'
}

install_app() {
  say "installing on $dname"
  xcrun devicectl device install app --device $udid $app >$tmp/install.txt 2>&1 || {
    cat $tmp/install.txt >&2
    if grep -qiE "developer mode" $tmp/install.txt; then
      print -u2 "  -> Turn on Developer Mode (Settings > Privacy & Security > Developer Mode), restart, retry."
    elif grep -qiE "untrusted|not trusted|profile has not been explicitly trusted" $tmp/install.txt; then
      print -u2 "  -> On the device: Settings > General > VPN & Device Management > the developer app > Trust."
    elif grep -qiE "locked" $tmp/install.txt; then
      print -u2 "  -> Unlock the device and retry."
    fi
    exit 1
  }
}

launch_app() {
  say "launching"
  xcrun devicectl device process launch --device $udid --terminate-existing $bundle >$tmp/launch.txt 2>&1 || {
    cat $tmp/launch.txt >&2
    if grep -qiE "untrusted|not been explicitly trusted|invalid code signature|Security" $tmp/launch.txt; then
      print -u2 "  -> On the device: Settings > General > VPN & Device Management > Apple Development: ... > Trust,"
      print -u2 "     then run again. (Only for a profile the device does not trust yet.)"
    elif grep -qiE "locked" $tmp/launch.txt; then
      print -u2 "  -> Unlock the device and retry."
    fi
    exit 1
  }
  say "running on $dname"
}

selftest() {
  local log=$root/build/ios-device-selftest.log
  say "self-test on $dname (the app shows its first view, checks it and quits; keep the device unlocked)"
  # The self-test prints "renderer OK ..." and a memory line, then exits (status 1/2 on failure).
  DEVICECTL_CHILD_ALMONDBREAD_SELFTEST=1 ${=commands[timeout]:+timeout 180} xcrun devicectl device process launch \
    --device $udid --terminate-existing --console $bundle --selftest-png=selftest.png >$log 2>&1
  grep -E "renderer|selftest" $log
  if ! grep -q "renderer OK" $log; then
    print -u2 "ios-device: self-test FAILED (log build/ios-device-selftest.log)"
    exit 1
  fi
  xcrun devicectl device copy from --device $udid --domain-type appDataContainer --domain-identifier $bundle \
    --source Documents/selftest.png --destination $root/build/ios-device-selftest.png -q \
    && say "self-test OK, PNG build/ios-device-selftest.png" \
    || { print -u2 "ios-device: self-test OK but the PNG could not be copied back"; exit 1; }
}

case $cmd in
  check)
    project
    say "compiling for iphoneos arm64, unsigned (no device needed)"
    xcodebuild -project $root/Almondbread.xcodeproj -target "Almondbread iOS" -sdk iphoneos -arch arm64 \
      -configuration $config SYMROOT=$root/.build/xcode-ios-check CODE_SIGNING_ALLOWED=NO -quiet build \
      || die "the device build does not compile"
    say "OK: $root/.build/xcode-ios-check/$config-iphoneos/Almondbread.app"
    ;;
  doctor)
    if has_account; then say "Xcode account: found"; else say "Xcode account: NONE (Xcode > Settings > Accounts)"; fi
    if has_identity; then say "Apple Development certificate: found"; else say "Apple Development certificate: none yet (made by the first signed build)"; fi
    find_device
    ;;
  run|selftest)
    check_account
    find_device
    project
    build_signed
    install_app
    if [[ $cmd == selftest ]]; then selftest; else launch_app; fi
    ;;
  *) die "usage: ios-device.sh check|doctor|run|selftest";;
esac
