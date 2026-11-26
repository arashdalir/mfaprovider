# TASK-0021: Scan QR codes with the camera

- Status: open
- Created: 2026-11-26
- Updated: 2026-11-26
- Related ADRs: ADR-0013 (QR decoding in the core), ADR-0002; new ADR for the frame path

## Goal
(User request, 2026-11-26.) In Add account, when the device has a camera, a "Scan with the camera" option shows the live picture and adds the account as soon as a QR code is read (also Google migration codes). Works on Android and on desktop webcams. Without a camera or without permission the option explains why and the other paths stay available.

## Acceptance criteria
- [ ] Android manifest declares `CAMERA` (feature not required); the WebView asks the user at first use
- [ ] UI: `getUserMedia` stream in a `<video>`, frames sampled a few times per second into a canvas, sent as JPEG/PNG base64 to the existing `draft_from_image` command; the decoder stays in Rust (no JS QR library)
- [ ] Stops the camera on success, Back, lock and unmount; never keeps frames
- [ ] Clear messages: no camera, permission denied, not secure context
- [ ] Tests with a fake `mediaDevices`; checked on the phone

## Decisions & notes (append-only, dated)
- 2026-11-26: Claude's decision (small enough for a note, recorded here): reuse `draft_from_image` per frame instead of a JS decoder or a new native plugin. Cost: base64 frames over IPC at ~3 fps; benefit: one decoder, one set of limits (ADR-0013), no new dependency. If it proves too slow on the phone, revisit with a native scanner plugin and an ADR.

## Progress log (append-only, dated)
- 2026-11-26: Task created.

## Next step
After TASK-0019 and TASK-0020: write `CameraStep.tsx` with a fake-camera test.
