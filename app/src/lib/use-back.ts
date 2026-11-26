import { useEffect, useRef } from "react";

import { pushBack } from "./back-stack";

/** While `onBack` is given, the system back runs it (see `back-stack`). `null` = at the root. */
export function useBack(onBack: (() => void) | null) {
  const latest = useRef(onBack);
  useEffect(() => {
    latest.current = onBack;
  });
  const active = onBack !== null;
  useEffect(() => {
    if (!active) {
      return;
    }
    return pushBack(() => latest.current?.());
  }, [active]);
}
