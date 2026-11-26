import { act, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { ApiContext, type VaultApi } from "@/ipc/api-context";
import { IpcCallError, type DraftView } from "@/ipc/client";
import { fakeApi } from "@/test/fake-api";

import { CameraStep, SCAN_INTERVAL_MS, describeCameraProblem } from "./CameraStep";

const draft = { draft_id: 1, issuer: "Acme", account: "alice" } as unknown as DraftView;
const frame = () => Promise.resolve(new Blob([new Uint8Array([1, 2, 3])], { type: "image/jpeg" }));
const noCodeYet = () =>
  new IpcCallError({ code: "invalid_input", message: "no QR code was found in the image" });

let track: { stop: ReturnType<typeof vi.fn> };

function allowCamera() {
  track = { stop: vi.fn() };
  const getUserMedia = vi.fn().mockResolvedValue({ getTracks: () => [track] });
  Object.defineProperty(navigator, "mediaDevices", { value: { getUserMedia }, configurable: true });
  return getUserMedia;
}

function open(overrides: Partial<VaultApi> = {}) {
  const api = fakeApi({ draftFromImage: vi.fn().mockResolvedValue([draft]), ...overrides });
  const onDrafts = vi.fn();
  const view = render(
    <ApiContext.Provider value={api}>
      <CameraStep onDrafts={onDrafts} grab={frame} />
    </ApiContext.Provider>,
  );
  return { api, onDrafts, view };
}

async function elapse(ms: number) {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(ms);
  });
}

beforeEach(() => {
  vi.useFakeTimers();
  Object.defineProperty(HTMLVideoElement.prototype, "videoWidth", { value: 640, configurable: true });
  Object.defineProperty(HTMLVideoElement.prototype, "videoHeight", { value: 480, configurable: true });
});

afterEach(() => {
  vi.useRealTimers();
  Reflect.deleteProperty(navigator, "mediaDevices");
});

describe("CameraStep", () => {
  it("asks for the rear camera and sends frames to the core until an account is found", async () => {
    const getUserMedia = allowCamera();
    const { api, onDrafts } = open();
    await elapse(0);
    expect(getUserMedia).toHaveBeenCalledWith({
      video: { facingMode: { ideal: "environment" } },
      audio: false,
    });
    expect(screen.getByRole("status")).toHaveTextContent("Hold the QR code");
    await elapse(SCAN_INTERVAL_MS);
    expect(api.draftFromImage).toHaveBeenCalledOnce();
    expect(onDrafts).toHaveBeenCalledWith([draft]);
  });

  it("stops the camera once an account was found and does not read further frames", async () => {
    allowCamera();
    const { api } = open();
    await elapse(SCAN_INTERVAL_MS * 4);
    expect(track.stop).toHaveBeenCalled();
    expect(api.draftFromImage).toHaveBeenCalledOnce();
  });

  it("keeps looking when a frame has no QR code, without complaining", async () => {
    allowCamera();
    const draftFromImage = vi.fn().mockRejectedValueOnce(noCodeYet()).mockResolvedValue([draft]);
    const { onDrafts } = open({ draftFromImage });
    await elapse(SCAN_INTERVAL_MS);
    expect(onDrafts).not.toHaveBeenCalled();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    await elapse(SCAN_INTERVAL_MS);
    expect(onDrafts).toHaveBeenCalledOnce();
  });

  it("tells the user when the code is not an authenticator code, and keeps looking", async () => {
    allowCamera();
    const draftFromImage = vi
      .fn()
      .mockRejectedValue(new IpcCallError({ code: "invalid_input", message: "the QR code is not an authenticator link" }));
    open({ draftFromImage });
    await elapse(SCAN_INTERVAL_MS * 2);
    expect(screen.getByText("the QR code is not an authenticator link")).toBeInTheDocument();
    expect(draftFromImage.mock.calls.length).toBeGreaterThan(1);
  });

  it("turns the camera off when the screen goes away", async () => {
    allowCamera();
    const { view } = open({ draftFromImage: vi.fn().mockRejectedValue(noCodeYet()) });
    await elapse(SCAN_INTERVAL_MS);
    view.unmount();
    expect(track.stop).toHaveBeenCalled();
  });

  it("explains a refused permission", async () => {
    Object.defineProperty(navigator, "mediaDevices", {
      value: { getUserMedia: vi.fn().mockRejectedValue(new DOMException("no", "NotAllowedError")) },
      configurable: true,
    });
    open();
    await elapse(0);
    expect(screen.getByRole("alert")).toHaveTextContent("The camera is not allowed");
  });

  it("explains when the device offers no camera API", async () => {
    open();
    await elapse(0);
    expect(screen.getByRole("alert")).toHaveTextContent("does not offer a camera");
  });
});

describe("describeCameraProblem", () => {
  it("has a sentence for each common failure", () => {
    expect(describeCameraProblem(new DOMException("", "NotFoundError"))).toBe("No camera was found.");
    expect(describeCameraProblem(new DOMException("", "NotReadableError"))).toBe(
      "The camera is in use by another app.",
    );
    expect(describeCameraProblem(new Error("x"))).toBe("The camera could not be started.");
  });
});
