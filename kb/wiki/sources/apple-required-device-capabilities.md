---
title: "Apple required device capabilities (UIRequiredDeviceCapabilities)"
type: source
authors: ["Apple"]
year: 2026
raw: "web"
status: digested
tags: [ios, ipad, device, app-store, gpu-families]
---

# Apple required device capabilities (UIRequiredDeviceCapabilities)

**URL:** <https://developer.apple.com/documentation/bundleresources/information-property-list/uirequireddevicecapabilities> and <https://developer.apple.com/support/required-device-capabilities/> · **Saved:** not saved (`raw: "web"`) · read 2026-11-05 through the page's JSON form

## What it is

Apple's list of the keys an app can require so the App Store does not offer it to devices without them.

## What it covers

- `metal`: "Support for graphics processing with Metal." Says nothing of a GPU family.
- `iphone-ipad-minimum-performance-a12`: "Performance and capabilities of the A12 Bionic and later chips."
- `iphone-performance-gaming-tier`: "the graphics performance and gaming features equivalent to the iPhone 15 Pro and iPhone 15 Pro Max."
- `ipad-minimum-performance-m1`: in the support page's key selector only; the reference page does not describe it.
- No key names A14 or a Metal GPU family.

## Reliability

Primary vendor reference for the keys. The support page's per-key device lists sit behind an interactive selector and were not read, so which devices each key covers is not confirmed beyond the wording above.

## Useful sections

The reference page's value list; the support page's key selector.

## Used by

- [iOS and iPadOS on a device](../platform/ios-device.md) "GPU floor"

# Citations

- <https://developer.apple.com/documentation/bundleresources/information-property-list/uirequireddevicecapabilities>
- <https://developer.apple.com/support/required-device-capabilities/>
