import { useEffect, useMemo, useState } from "react";
import { Cloud, DownloadCloud, Loader2, Save, UploadCloud, Wifi } from "lucide-react";
import { confirm as dialogConfirm } from "@tauri-apps/plugin-dialog";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { CustomSelect } from "../CustomSelect";
import * as api from "../../lib/tauri";
import { cn } from "../../utils";

type PresetId = "custom" | "jianguoyun" | "nextcloud" | "synology";
type BusyState = "idle" | "loading" | "testing" | "saving" | "uploading" | "downloading";

const PRESETS: Array<{ id: PresetId; baseUrl: string; labelKey: string }> = [
  { id: "custom", baseUrl: "", labelKey: "settings.webdavSync.presetCustom" },
  { id: "jianguoyun", baseUrl: "https://dav.jianguoyun.com/dav/", labelKey: "settings.webdavSync.presetJianGuoYun" },
  { id: "nextcloud", baseUrl: "https://your-server/remote.php/dav/files/USERNAME/", labelKey: "settings.webdavSync.presetNextcloud" },
  { id: "synology", baseUrl: "http://your-nas-ip:5005/", labelKey: "settings.webdavSync.presetSynology" },
];

const defaultSettings: api.WebDavSyncSettings = {
  enabled: false,
  baseUrl: "",
  username: "",
  password: "",
  remoteRoot: "skills-manager-plus-sync",
  profile: "default",
};

function getErrorMessage(error: unknown) {
  if (error && typeof error === "object" && "message" in error) {
    return String((error as { message?: unknown }).message);
  }
  return String(error);
}

function normalizeSettings(settings: api.WebDavSyncSettings | null): api.WebDavSyncSettings {
  return {
    ...defaultSettings,
    ...(settings ?? {}),
    password: settings?.password ?? "",
  };
}

function formatDateTime(value?: string | number | null) {
  if (!value) return "";
  const date = typeof value === "number" ? new Date(value) : new Date(value);
  if (Number.isNaN(date.getTime())) return String(value);
  return date.toLocaleString();
}

function selectedPreset(baseUrl: string): PresetId {
  const preset = PRESETS.find((item) => item.baseUrl && item.baseUrl === baseUrl);
  return preset?.id ?? "custom";
}

export function WebDavSyncSection() {
  const { t } = useTranslation();
  const [settings, setSettings] = useState<api.WebDavSyncSettings>(defaultSettings);
  const [dirty, setDirty] = useState(false);
  const [passwordTouched, setPasswordTouched] = useState(false);
  const [busy, setBusy] = useState<BusyState>("loading");
  const [remoteInfo, setRemoteInfo] = useState<api.RemoteSnapshotInfo | null>(null);
  const [presetId, setPresetId] = useState<PresetId>("custom");

  const actionButtonClass =
    "inline-flex h-8 items-center gap-1.5 rounded-[4px] border px-2.5 text-[13px] font-medium transition-colors outline-none disabled:cursor-not-allowed disabled:opacity-60";
  const fieldClass =
    "h-8 rounded-[4px] border border-border-subtle bg-background px-2.5 text-[13px] text-secondary outline-none transition-colors focus:border-border disabled:cursor-not-allowed disabled:opacity-60";
  const segmentedButtonClass =
    "flex h-8 items-center gap-1.5 px-2.5 rounded-[3px] text-[13px] font-medium transition-colors outline-none disabled:cursor-not-allowed disabled:opacity-60";

  useEffect(() => {
    let cancelled = false;
    api.webdavSyncGetSettings()
      .then((value) => {
        if (!cancelled) {
          const normalized = normalizeSettings(value);
          setSettings(normalized);
          setPresetId(selectedPreset(normalized.baseUrl));
        }
      })
      .catch((error) => {
        if (!cancelled) toast.error(getErrorMessage(error));
      })
      .finally(() => {
        if (!cancelled) setBusy("idle");
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const presetOptions = useMemo(
    () => PRESETS.map((preset) => ({ value: preset.id, label: t(preset.labelKey) })),
    [t]
  );

  const updateSettings = (patch: Partial<api.WebDavSyncSettings>) => {
    setSettings((prev) => ({ ...prev, ...patch }));
    setRemoteInfo(null);
    setDirty(true);
  };

  const run = async (state: BusyState, action: () => Promise<void>) => {
    setBusy(state);
    try {
      await action();
    } catch (error) {
      toast.error(getErrorMessage(error));
    } finally {
      setBusy("idle");
    }
  };

  const handlePresetChange = (presetId: string) => {
    const preset = PRESETS.find((item) => item.id === presetId);
    if (!preset) return;
    setPresetId(preset.id);
    if (preset.id === "custom") return;
    updateSettings({ baseUrl: preset.baseUrl });
  };

  const handleSave = () => run("saving", async () => {
    await api.webdavSyncSaveSettings(settings, passwordTouched);
    setDirty(false);
    setPasswordTouched(false);
    toast.success(t("settings.webdavSync.saveSuccess"));
  });

  const handleTest = () => run("testing", async () => {
    await api.webdavTestConnection(settings, !passwordTouched);
    toast.success(t("settings.webdavSync.testSuccess"));
  });

  const handleUpload = () => run("uploading", async () => {
    const info = await api.webdavSyncFetchRemoteInfo();
    setRemoteInfo(info);
    const confirmed = await dialogConfirm(t("settings.webdavSync.uploadConfirmBody"), {
      title: t("settings.webdavSync.uploadConfirmTitle"),
      kind: "warning",
    });
    if (!confirmed) return;
    await api.webdavSyncUpload();
    const nextInfo = await api.webdavSyncFetchRemoteInfo().catch(() => null);
    if (nextInfo) setRemoteInfo(nextInfo);
    toast.success(t("settings.webdavSync.uploadSuccess"));
  });

  const handleDownload = () => run("downloading", async () => {
    const info = await api.webdavSyncFetchRemoteInfo();
    setRemoteInfo(info);
    if (info.empty) {
      toast.info(t("settings.webdavSync.remoteEmpty"));
      return;
    }
    if (!info.compatible) {
      toast.error(t("settings.webdavSync.remoteIncompatible"));
      return;
    }
    const confirmed = await dialogConfirm(t("settings.webdavSync.downloadConfirmBody"), {
      title: t("settings.webdavSync.downloadConfirmTitle"),
      kind: "warning",
    });
    if (!confirmed) return;
    await api.webdavSyncDownload();
    toast.success(t("settings.webdavSync.downloadSuccess"));
    window.setTimeout(() => window.location.reload(), 900);
  });

  const isBusy = busy !== "idle";
  const enabled = settings.enabled;
  const canUseConnection = enabled && !dirty && !isBusy;
  const canTest = enabled && !dirty && !isBusy;
  const canSave = dirty && !isBusy;
  const lastSyncAt = formatDateTime(settings.status?.lastSyncAt);

  return (
    <section>
      <h2 className="app-section-title mb-3">
        {t("settings.webdavSync.title")}
      </h2>
      <div className="app-panel overflow-hidden divide-y divide-border-subtle">
        <div className="px-4 py-3">
          <div className="flex flex-wrap items-start justify-between gap-3">
            <div className="min-w-0 flex-1">
              <h3 className="flex items-center gap-1.5 text-[13px] text-secondary font-medium mb-0.5">
                <Cloud className="h-3.5 w-3.5 text-accent" />
                {t("settings.webdavSync.title")}
              </h3>
              <p className="text-[13px] text-muted">{t("settings.webdavSync.description")}</p>
            </div>
            <div className="flex flex-wrap rounded-[4px] border border-border-subtle bg-background p-px">
              <button
                type="button"
                onClick={() => updateSettings({ enabled: true })}
                disabled={busy === "loading"}
                className={cn(
                  segmentedButtonClass,
                  enabled ? "bg-surface-active text-secondary" : "text-muted hover:text-tertiary"
                )}
              >
                {t("settings.webdavSync.enabled")}
              </button>
              <button
                type="button"
                onClick={() => updateSettings({ enabled: false })}
                disabled={busy === "loading"}
                className={cn(
                  segmentedButtonClass,
                  !enabled ? "bg-surface-active text-secondary" : "text-muted hover:text-tertiary"
                )}
              >
                {t("settings.webdavSync.disabled")}
              </button>
            </div>
          </div>

          {!enabled ? (
            <div className="mt-3 flex flex-wrap items-center justify-between gap-2 rounded-[4px] border border-dashed border-border-subtle px-3 py-3">
              <p className="text-[13px] text-muted">{t("settings.webdavSync.disabledHint")}</p>
              {dirty && (
                <button
                  type="button"
                  onClick={() => void handleSave()}
                  disabled={!canSave}
                  className={`${actionButtonClass} bg-surface-hover hover:bg-surface-active text-tertiary border-border`}
                >
                  {busy === "saving" ? <Loader2 className="h-3 w-3 animate-spin" /> : <Save className="h-3 w-3" />}
                  {t("settings.webdavSync.save")}
                </button>
              )}
            </div>
          ) : (
            <div className="mt-4 space-y-4">
              <div className="grid gap-3 md:grid-cols-2">
                <label className="space-y-1.5">
                  <span className="text-[12px] text-muted">{t("settings.webdavSync.preset")}</span>
                  <CustomSelect
                    value={presetId}
                    onChange={handlePresetChange}
                    options={presetOptions}
                  />
                </label>
                <label className="space-y-1.5">
                  <span className="text-[12px] text-muted">{t("settings.webdavSync.baseUrl")}</span>
                  <input
                    type="text"
                    value={settings.baseUrl}
                    onChange={(event) => {
                      setPresetId(selectedPreset(event.target.value));
                      updateSettings({ baseUrl: event.target.value });
                    }}
                    placeholder={t("settings.webdavSync.baseUrlPlaceholder")}
                    className={`${fieldClass} w-full font-mono`}
                  />
                </label>
                <label className="space-y-1.5">
                  <span className="text-[12px] text-muted">{t("settings.webdavSync.username")}</span>
                  <input
                    type="text"
                    value={settings.username}
                    onChange={(event) => updateSettings({ username: event.target.value })}
                    className={`${fieldClass} w-full`}
                  />
                </label>
                <label className="space-y-1.5">
                  <span className="text-[12px] text-muted">{t("settings.webdavSync.password")}</span>
                  <input
                    type="password"
                    value={settings.password ?? ""}
                    onChange={(event) => {
                      setPasswordTouched(true);
                      updateSettings({ password: event.target.value });
                    }}
                    className={`${fieldClass} w-full`}
                  />
                </label>
                <label className="space-y-1.5">
                  <span className="text-[12px] text-muted">{t("settings.webdavSync.remoteRoot")}</span>
                  <input
                    type="text"
                    value={settings.remoteRoot}
                    onChange={(event) => updateSettings({ remoteRoot: event.target.value })}
                    className={`${fieldClass} w-full font-mono`}
                  />
                </label>
                <label className="space-y-1.5">
                  <span className="text-[12px] text-muted">{t("settings.webdavSync.profile")}</span>
                  <input
                    type="text"
                    value={settings.profile}
                    onChange={(event) => updateSettings({ profile: event.target.value })}
                    className={`${fieldClass} w-full font-mono`}
                  />
                </label>
              </div>

              {(dirty || lastSyncAt || settings.status?.lastError) && (
                <div className="space-y-1 rounded-[4px] bg-surface-hover px-3 py-2 text-[12px]">
                  {dirty && <p className="text-amber-600 dark:text-amber-300">{t("settings.webdavSync.saveBeforeSync")}</p>}
                  {lastSyncAt && <p className="text-muted">{t("settings.webdavSync.lastSyncAt", { time: lastSyncAt })}</p>}
                  {settings.status?.lastError && (
                    <p className="text-red-500">{t("settings.webdavSync.lastError", { error: settings.status.lastError })}</p>
                  )}
                </div>
              )}

              {remoteInfo && (
                <div className="rounded-[4px] border border-border-subtle bg-background px-3 py-2 text-[12px] text-muted">
                  {remoteInfo.empty ? (
                    <p>{t("settings.webdavSync.remoteEmpty")}</p>
                  ) : (
                    <div className="space-y-1">
                      <p className={remoteInfo.compatible ? "text-muted" : "text-red-500"}>
                        {remoteInfo.compatible
                          ? t("settings.webdavSync.remoteCompatible")
                          : t("settings.webdavSync.remoteIncompatible")}
                      </p>
                      <p>
                        {[
                          remoteInfo.deviceName,
                          formatDateTime(remoteInfo.createdAt),
                          remoteInfo.snapshotId,
                        ].filter(Boolean).join(" · ")}
                      </p>
                      {remoteInfo.remotePath && <p className="font-mono">{remoteInfo.remotePath}</p>}
                      {remoteInfo.artifacts.length > 0 && (
                        <p>{t("settings.webdavSync.remoteArtifacts", { artifacts: remoteInfo.artifacts.join(", ") })}</p>
                      )}
                    </div>
                  )}
                </div>
              )}

              <div className="flex flex-wrap gap-2">
                <button
                  type="button"
                  onClick={() => void handleTest()}
                  disabled={!canTest}
                  className={`${actionButtonClass} bg-surface-hover hover:bg-surface-active text-tertiary border-border`}
                >
                  {busy === "testing" ? <Loader2 className="h-3 w-3 animate-spin" /> : <Wifi className="h-3 w-3" />}
                  {t("settings.webdavSync.test")}
                </button>
                <button
                  type="button"
                  onClick={() => void handleSave()}
                  disabled={!canSave}
                  className={`${actionButtonClass} bg-accent text-white border-accent hover:opacity-90`}
                >
                  {busy === "saving" ? <Loader2 className="h-3 w-3 animate-spin" /> : <Save className="h-3 w-3" />}
                  {t("settings.webdavSync.save")}
                </button>
                <button
                  type="button"
                  onClick={() => void handleUpload()}
                  disabled={!canUseConnection}
                  className={`${actionButtonClass} bg-surface-hover hover:bg-surface-active text-tertiary border-border`}
                >
                  {busy === "uploading" ? <Loader2 className="h-3 w-3 animate-spin" /> : <UploadCloud className="h-3 w-3" />}
                  {t("settings.webdavSync.upload")}
                </button>
                <button
                  type="button"
                  onClick={() => void handleDownload()}
                  disabled={!canUseConnection}
                  className={`${actionButtonClass} bg-surface-hover hover:bg-surface-active text-tertiary border-border`}
                >
                  {busy === "downloading" ? <Loader2 className="h-3 w-3 animate-spin" /> : <DownloadCloud className="h-3 w-3" />}
                  {t("settings.webdavSync.download")}
                </button>
              </div>
            </div>
          )}
        </div>
      </div>
    </section>
  );
}
