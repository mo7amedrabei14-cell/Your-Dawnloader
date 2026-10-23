# Platform support and release matrix

## Desktop targets from the beginning

| Platform | Architecture | Target artifact | Validation |
|---|---|---|---|
| Windows 10/11 | x64 | installer + optional portable ZIP | native Windows runner |
| Windows | x86 | 32-bit installer where all dependencies support it | dedicated x86 target and smoke test |
| macOS | Apple Silicon arm64 | signed/notarized DMG/app | Apple Silicon/macOS runner |
| macOS | Intel x86_64 | signed/notarized DMG/app | Intel runner or verified cross-target runtime |
| Linux | x86_64 | AppImage and maintained distro packages | Linux runner with WebKitGTK |
| Linux | aarch64 | supported package/AppImage | ARM64 runner or verified cross-build + runtime smoke |

## Extensions
- Chrome/Chromium and Edge: build, install/update, browser tests.
- Firefox: target build and WebExtension API tests.
- Safari macOS: conversion/packaging/signing on Apple hardware; API parity must be checked.
- Mobile browsers are not implied by desktop extension support.

## Mobile
Android and iOS are architectural targets but not released until share-sheet, background transfer, permission, store policy, signing, and physical-device tests pass.

## Release statuses
Label every platform planned, build-tested, smoke-tested, signed/notarized, or released. Do not claim universal support before all gates pass.

Windows signing needs a suitable certificate for trusted publisher identity. macOS distribution commonly needs Apple signing/notarization credentials. Never expose secrets to untrusted CI workflows.
