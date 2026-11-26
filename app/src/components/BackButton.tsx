import { ArrowLeft } from "lucide-react";

import { Button } from "@/components/ui/button";
import { goBack } from "@/lib/back-stack";

export function BackButton() {
  return (
    <Button variant="ghost" className="-ml-2 self-start" onClick={goBack}>
      <ArrowLeft /> Back
    </Button>
  );
}
