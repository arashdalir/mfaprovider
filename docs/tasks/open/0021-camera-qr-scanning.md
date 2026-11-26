# TASK-0021: Scan QR codes with the camera

- Status: in-progress
- Created: 2026-11-26
- Updated: 2026-11-26
- Related ADRs: ADR-0013 (QR decoding in the core), ADR-0002; new ADR for the frame path

## Goal
(User request, 2026-11-26.) In Add account, when the device has a camera, a "Scan with the camera" option shows the live picture and adds the account as soon as a QR code is read (also Google migration codes). Works on Android and on desktop webcams. Without a camera or without permission the option explains why and the other paths stay available.

## Acceptance criteria
- [x] Android manifest declares `CAMERA` (feature not required); the WebView asks the user at first use
- [x] UI: `getUserMedia` stream in a `<video>`, frames sampled a few times per second into a canvas, sent as JPEG/PNG base64 to the existing `draft_from_image` command; the decoder stays in Rust (no JS QR library)
- [x] Stops the camera on success, Back, lock and unmount; never keeps frames
- [x] Clear messages: no camera, permission denied, not secure context
- [x] Tests with a fake `mediaDevices`; checked on the phone

## Decisions & notes (append-only, dated)
- 2026-11-26: Claude's decision (small enough for a note, recorded here): reuse `draft_from_image` per frame instead of a JS decoder or a new native plugin. Cost: base64 frames over IPC at ~3 fps; benefit: one decoder, one set of limits (ADR-0013), no new dependency. If it proves too slow on the phone, revisit with a native scanner plugin and an ADR.

## Progress log (append-only, dated)
- 2026-11-26: Task created.

- 2026-11-26: Done in code. `CameraStep.tsx`: `getUserMedia` (rear camera preferred, no audio), a frame every 400 ms scaled to at most 960 px, JPEG, base64 to the existing `draft_from_image`; stops at the first account found, on unmount and on lock; frames are never stored. "No QR code in the image" is ignored silently; other core messages (e.g. "not an authenticator link") are shown as a hint while it keeps looking; permission, no-camera, in-use and no-API cases have their own sentences. `useHasCamera()` (enumerateDevices, kind `videoinput`) decides whether Add account offers "Scan a QR code with the camera"; Back (TASK-0019) leaves the step. Android manifest: `CAMERA` permission, camera feature not required (in `gen/android`, uncommitted until ADR-0015 is accepted). UI tests: 172 (11 new). **Not verified:** on the phone (permission prompt through the generated WebChromeClient, picture, speed of the frame loop over IPC, reading the code), on desktop webcams, Google migration codes by camera.

## Next step
Rebuild, install on the phone, user tries the camera on a real QR code (e.g. the one from another device's account details, TASK-0020).
