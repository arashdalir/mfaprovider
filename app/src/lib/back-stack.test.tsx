import { renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { goBack, resetBackStack } from "./back-stack";
import { useBack } from "./use-back";

const depth = () => (window.history.state === null ? 0 : 1);

async function systemBack() {
  goBack();
  await new Promise((resolve) => setTimeout(resolve, 20));
}

beforeEach(() => {
  resetBackStack();
});

describe("useBack", () => {
  it("runs the handler when the system goes back", async () => {
    const onBack = vi.fn();
    renderHook(() => useBack(onBack));
    await systemBack();
    expect(onBack).toHaveBeenCalledOnce();
  });

  it("does nothing while no handler is given", async () => {
    const { result } = renderHook(() => useBack(null));
    expect(result.current).toBeUndefined();
    expect(depth()).toBe(0);
  });

  it("runs the most recent handler first, then the one below", async () => {
    const calls: string[] = [];
    renderHook(() => useBack(() => calls.push("outer")));
    renderHook(() => useBack(() => calls.push("inner")));
    await systemBack();
    await systemBack();
    expect(calls).toEqual(["inner", "outer"]);
  });

  it("gives its history entry back when the screen closes by itself", async () => {
    const onBack = vi.fn();
    const { unmount } = renderHook(() => useBack(onBack));
    unmount();
    await waitFor(() => expect(depth()).toBe(0));
    expect(onBack).not.toHaveBeenCalled();
  });

  it("does not run a handler again after it was used", async () => {
    const onBack = vi.fn();
    const { unmount } = renderHook(() => useBack(onBack));
    await systemBack();
    unmount();
    await systemBack();
    expect(onBack).toHaveBeenCalledOnce();
  });

  it("always runs the latest handler without re-registering", async () => {
    const first = vi.fn();
    const second = vi.fn();
    const { rerender } = renderHook(({ handler }) => useBack(handler), {
      initialProps: { handler: first },
    });
    rerender({ handler: second });
    await systemBack();
    expect(first).not.toHaveBeenCalled();
    expect(second).toHaveBeenCalledOnce();
  });
});
