import * as Dialog from "@radix-ui/react-dialog";
import { X } from "lucide-react";
import { useState, type ReactElement } from "react";
import { useTranslation } from "react-i18next";
import { ipc } from "@/lib/ipc";

export function ProfileAccessDialog({
  browserName,
  onRetry,
  children,
}: {
  browserName: string;
  onRetry: () => Promise<void> | void;
  children: ReactElement;
}) {
  const { t } = useTranslation("browsers");
  const [open, setOpen] = useState(false);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const openSettings = async () => {
    setPending(true);
    setError(null);
    try {
      await ipc.openProfilePrivacySettings();
    } catch (error) {
      setError(String(error));
    } finally {
      setPending(false);
    }
  };

  const retry = async () => {
    setPending(true);
    setError(null);
    try {
      await onRetry();
      setOpen(false);
    } catch (error) {
      setError(String(error));
    } finally {
      setPending(false);
    }
  };

  return (
    <Dialog.Root open={open} onOpenChange={setOpen}>
      <Dialog.Trigger asChild>{children}</Dialog.Trigger>
      <Dialog.Portal>
        <Dialog.Overlay className="profile-order-modal-overlay" />
        <Dialog.Content className="profile-order-modal profile-access-modal">
          <div className="profile-order-modal-head">
            <div>
              <Dialog.Title className="profile-order-modal-title">
                {t("profileAccess.title", { name: browserName })}
              </Dialog.Title>
              <Dialog.Description className="profile-order-modal-desc">
                {t("profileAccess.description", { name: browserName })}
              </Dialog.Description>
            </div>
            <Dialog.Close asChild>
              <button
                type="button"
                className="profile-order-icon-btn"
                aria-label={t("profileAccess.close")}
              >
                <X size={16} />
              </button>
            </Dialog.Close>
          </div>
          <div className="profile-access-instructions">
            <p>{t("profileAccess.instructions", { name: browserName })}</p>
            <p className="mac-muted">{t("profileAccess.afterAllow")}</p>
            {error && (
              <p role="alert" className="profile-order-error">{error}</p>
            )}
          </div>
          <div className="profile-order-modal-footer">
            <Dialog.Close asChild>
              <button type="button" className="mac-tbtn">
                {t("profileAccess.later")}
              </button>
            </Dialog.Close>
            <span className="grow" />
            <button
              type="button"
              className="mac-tbtn"
              disabled={pending}
              onClick={() => void retry()}
            >
              {t("rescan")}
            </button>
            <button
              type="button"
              className="mac-tbtn primary"
              disabled={pending}
              onClick={() => void openSettings()}
            >
              {t("profileAccess.openSettings")}
            </button>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
