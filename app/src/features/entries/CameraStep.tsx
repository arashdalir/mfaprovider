import { useEffect, useRef, useState } from "react";

import { useApi } from "@/ipc/api-context";
import type { DraftView } from "@/ipc/client";
import { describeError } from "@/ipc/error-message";

import { fileToBase64 } from "./file-to-base64";

export const SCAN_INTERVAL_MS = 400;
const MAX_FRAME_SIDE = 960;
const NO_CODE_IN_FRAME = "no QR code was found in the image";

export type FrameGrabber = (video: HTMLVideoElement) => Promise<Blob | null>;

/** A JPEG of what the camera sees now, scaled down so each frame stays small. */
export const grabFrame: FrameGrabber = (video) => {
  const scale = Math.min(1, MAX_FRAME_SIDE / Math.max(video.videoWidth, video.videoHeight));
  const canvas = document.createElement("canvas");
  canvas.width = Math.round(video.videoWidth * scale);
  canvas.height = Math.round(video.videoHeight * scale);
  canvas.getContext("2d")?.drawImage(video, 0, 0, canvas.width, canvas.height);
  return new Promise((resolve) => canvas.toBlob(resolve, "image/jpeg", 0.8));
};

export function describeCameraProblem(error: unknown): string {
  switch (error instanceof DOMException ? error.name : "") {
    case "NotAllowedError":
    case "SecurityError":
      return "The camera is not allowed. Allow it for this app in the system settings, then try again.";
    case "NotFoundError":
    case "OverconstrainedError":
      return "No camera was found.";
    case "NotReadableError":
      return "The camera is in use by another app.";
    default:
      return "The camera could not be started.";
  }
}

/** True when the device reports at least one camera. */
export function useHasCamera(): boolean {
  const [found, setFound] = useState(false);
  useEffect(() => {
    let cancelled = false;
    navigator.mediaDevices
      ?.enumerateDevices()
      .then((devices) => !cancelled && setFound(devices.some((d) => d.kind === "videoinput")))
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, []);
  return found;
}

/**
 * Shows the camera and hands every few frames to the core, which is the only place a QR code
 * is read. Stops as soon as an account is found, and whenever the screen goes away.
 */
export function CameraStep({
  onDrafts,
  grab = grabFrame,
}: {
  onDrafts: (drafts: DraftView[]) => void;
  grab?: FrameGrabber;
}) {
  const api = useApi();
  const video = useRef<HTMLVideoElement>(null);
  const found = useRef(onDrafts);
  const [problem, setProblem] = useState<string | null>(null);
  const [hint, setHint] = useState<string | null>(null);
  const [ready, setReady] = useState(false);

  useEffect(() => {
    found.current = onDrafts;
  });

  useEffect(() => {
    let stopped = false;
    let stream: MediaStream | null = null;
    let timer: number | undefined;
    let reading = false;

    const stop = () => {
      stopped = true;
      window.clearInterval(timer);
      stream?.getTracks().forEach((track) => track.stop());
    };

    async function readFrame(element: HTMLVideoElement) {
      if (reading || stopped || element.videoWidth === 0) {
        return;
      }
      reading = true;
      try {
        const blob = await grab(element);
        if (blob === null || stopped) {
          return;
        }
        const image = await fileToBase64(new File([blob], "frame.jpg", { type: blob.type }));
        const drafts = await api.draftFromImage(image);
        if (!stopped) {
          stop();
          found.current(drafts);
        }
      } catch (error) {
        const message = describeError(error);
        setHint(message.toLowerCase().includes(NO_CODE_IN_FRAME) ? null : message);
      } finally {
        reading = false;
      }
    }

    async function start() {
      if (!navigator.mediaDevices?.getUserMedia) {
        setProblem("This device does not offer a camera to the app.");
        return;
      }
      try {
        stream = await navigator.mediaDevices.getUserMedia({
          video: { facingMode: { ideal: "environment" } },
          audio: false,
        });
      } catch (error) {
        setProblem(describeCameraProblem(error));
        return;
      }
      const element = video.current;
      if (stopped || element === null) {
        stream.getTracks().forEach((track) => track.stop());
        return;
      }
      element.srcObject = stream;
      try {
        await element.play();
      } catch {
        // autoplay is on; a refused play() only means the user has to wait for the picture
      }
      setReady(true);
      timer = window.setInterval(() => void readFrame(element), SCAN_INTERVAL_MS);
    }

    void start();
    return stop;
  }, [api, grab]);

  return (
    <section aria-label="Camera" className="flex flex-col gap-4">
      <h2 className="text-lg font-semibold">Scan with the camera</h2>
      {problem ? (
        <p role="alert" className="text-sm text-destructive">
          {problem}
        </p>
      ) : (
        <>
          <video
            ref={video}
            aria-label="Camera picture"
            className="w-full rounded-md bg-black"
            playsInline
            muted
          />
          <p role="status" className="text-sm text-muted-foreground">
            {ready ? "Hold the QR code in front of the camera." : "Starting the camera…"}
          </p>
          {hint && <p className="text-sm text-muted-foreground">{hint}</p>}
        </>
      )}
    </section>
  );
}
