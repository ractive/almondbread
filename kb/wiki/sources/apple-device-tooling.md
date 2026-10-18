---
title: "Apple device tooling and iOS process APIs (devicectl, xcodebuild provisioning, os/proc.h)"
type: source
status: digested
raw: "web"
authors: [Apple]
year: 2026
tags: [ios, ipad, device, signing, memory, tooling]
---

# Apple device tooling and iOS process APIs

**What:** the command-line help of Xcode 26.6's device tools as installed (`xcrun devicectl
… --help`, `xcodebuild -help`) and the iPhoneOS 26 SDK header `usr/include/os/proc.h`. Read
2026-10-18; not saved (local tool output and SDK headers; Apple documentation at the URL
above).

## Facts used

- `xcrun devicectl list devices --json-output f` lists paired devices (the JSON file is "the
  ONLY supported interface for scripts"); `device install app --device <id> <app>`;
  `device process launch --device <id> [--terminate-existing] [--console] <bundle> [args]`:
  `--console` attaches the app's stdio and waits for it to exit; environment variables go
  in with a `DEVICECTL_CHILD_` prefix or `--environment-variables '{json}'`.
  `device copy from --domain-type appDataContainer --domain-identifier <bundle> --source
  <path in container> --destination <local>` fetches a file from the app's container.
- `xcodebuild -allowProvisioningUpdates`: "for automatically signed targets, xcodebuild will
  create and update profiles, app IDs, and certificates"; it "requires a developer account
  to have been added in Xcode's Accounts settings" (or an App Store Connect API key).
  `-allowProvisioningDeviceRegistration` registers the destination device with the team
  (only together with `-allowProvisioningUpdates`).
- `os_proc_available_memory()` (iOS 13+, unavailable on macOS) returns the bytes the app
  may still allocate before reaching its limit; "0 is returned if the calling process is not
  an app, or the calling process exceeds its memory limit". The iOS simulator returns 0
  (measured 2026-10-18: no per-app limit there).

Used by [[platform/ios-device]].
