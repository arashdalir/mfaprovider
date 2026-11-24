import { fireEvent, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { ApiContext } from "@/ipc/api-context";
import { fakeApi } from "@/test/fake-api";

import { useActivityReporter } from "./useActivityReporter";

function Probe() {
  useActivityReporter();
  return null;
}

describe("useActivityReporter", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("reports activity, at most once per five seconds", () => {
    const api = fakeApi();
    render(
      <ApiContext.Provider value={api}>
        <Probe />
      </ApiContext.Provider>,
    );
    fireEvent.pointerDown(window);
    fireEvent.keyDown(window);
    expect(api.recordActivity).toHaveBeenCalledTimes(1);
    vi.advanceTimersByTime(5000);
    fireEvent.keyDown(window);
    expect(api.recordActivity).toHaveBeenCalledTimes(2);
  });

  it("stops listening when unmounted", () => {
    const api = fakeApi();
    const view = render(
      <ApiContext.Provider value={api}>
        <Probe />
      </ApiContext.Provider>,
    );
    view.unmount();
    fireEvent.keyDown(window);
    expect(api.recordActivity).not.toHaveBeenCalled();
  });

  it("ignores a failing report", () => {
    const api = fakeApi({ recordActivity: vi.fn().mockRejectedValue(new Error("locked")) });
    render(
      <ApiContext.Provider value={api}>
        <Probe />
      </ApiContext.Provider>,
    );
    expect(() => fireEvent.keyDown(window)).not.toThrow();
  });
});
