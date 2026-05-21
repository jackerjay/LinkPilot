// Compact button that opens the native macOS "Choose Application" dialog
// and hands the picked app's display name (and optional bundle id) back
// to the parent. Used next to source-app text inputs in RuleEditor and
// the Test-URL panel so users don't have to remember exact app names.

import { useState } from "react";
import { FolderSearch2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { ipc } from "@/lib/ipc";

interface Props {
  onPicked: (picked: { name: string; bundleId: string }) => void;
  tooltip?: string;
}

export function AppPickerButton({
  onPicked,
  tooltip,
}: Props) {
  const { t } = useTranslation("common");
  const label = tooltip ?? t("appPicker.tooltip");
  const [busy, setBusy] = useState(false);
  const open = async () => {
    setBusy(true);
    try {
      const result = await ipc.pickApp();
      if (result) {
        onPicked({ name: result.name, bundleId: result.bundle_id });
      }
    } finally {
      setBusy(false);
    }
  };
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          variant="outline"
          size="icon"
          onClick={open}
          disabled={busy}
          aria-label={label}
        >
          <FolderSearch2 />
        </Button>
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  );
}
