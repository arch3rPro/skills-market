import { useState, useEffect, useCallback } from "react";
import {
  Search,
  Package,
  Check,
  Loader2,
  Plus,
  Store,
  HardDrive,
  Globe,
  RefreshCw,
  Trash2,
  ExternalLink,
  X,
  Layers,
  DownloadCloud,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { cn } from "../utils";
import { useApp } from "../context/AppContext";
import * as api from "../lib/tauri";
import type {
  PluginWithMarketDto,
  PluginMarketRecord,
  PluginInstalledSkillDto,
  BatchPluginInstallResult,
} from "../lib/tauri";
import { getErrorMessage } from "../lib/error";

type SubTab = "discover" | "installed" | "markets";

export function PluginMarketplace() {
  const { t } = useTranslation();
  const { refreshManagedSkills } = useApp();
  const [subTab, setSubTab] = useState<SubTab>("discover");

  const [plugins, setPlugins] = useState<PluginWithMarketDto[]>([]);
  const [pluginsLoading, setPluginsLoading] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [installingPlugin, setInstallingPlugin] = useState<string | null>(null);
  const [installedPluginNames, setInstalledPluginNames] = useState<Set<string>>(new Set());

  const [installedSkills, setInstalledSkills] = useState<PluginInstalledSkillDto[]>([]);
  const [installedLoading, setInstalledLoading] = useState(false);

  const [markets, setMarkets] = useState<PluginMarketRecord[]>([]);
  const [marketsLoading, setMarketsLoading] = useState(false);
  const [refreshingMarketId, setRefreshingMarketId] = useState<string | null>(null);

  const [detailPlugin, setDetailPlugin] = useState<PluginWithMarketDto | null>(null);
  const [addMarketOpen, setAddMarketOpen] = useState(false);

  const loadPlugins = useCallback(async () => {
    setPluginsLoading(true);
    try {
      const result = searchQuery.trim()
        ? await api.searchPlugins(searchQuery.trim())
        : await api.listAllPlugins();
      setPlugins(result);
    } catch (e) {
      toast.error(getErrorMessage(e, t("common.error")));
    } finally {
      setPluginsLoading(false);
    }
  }, [searchQuery, t]);

  const loadInstalledSkills = useCallback(async () => {
    setInstalledLoading(true);
    try {
      const result = await api.listPluginInstalledSkills();
      setInstalledSkills(result);
      const names = new Set<string>();
      result.forEach((s) => names.add(s.plugin_name));
      setInstalledPluginNames(names);
    } catch (e) {
      toast.error(getErrorMessage(e, t("common.error")));
    } finally {
      setInstalledLoading(false);
    }
  }, [t]);

  const loadMarkets = useCallback(async () => {
    setMarketsLoading(true);
    try {
      const result = await api.listPluginMarkets();
      setMarkets(result);
    } catch (e) {
      toast.error(getErrorMessage(e, t("common.error")));
    } finally {
      setMarketsLoading(false);
    }
  }, [t]);

  useEffect(() => {
    loadPlugins();
    loadInstalledSkills();
    loadMarkets();
  }, [loadPlugins, loadInstalledSkills, loadMarkets]);

  useEffect(() => {
    const timer = setTimeout(() => {
      loadPlugins();
    }, 400);
    return () => clearTimeout(timer);
  }, [searchQuery, loadPlugins]);

  const handleInstallPlugin = async (plugin: PluginWithMarketDto) => {
    setInstallingPlugin(plugin.name);
    const toastId = toast.loading(t("install.plugin.installing"));
    try {
      const result: BatchPluginInstallResult = await api.installPluginSkills(
        plugin.market_id,
        plugin.name,
      );
      await refreshManagedSkills();
      await loadInstalledSkills();
      if (result.failed.length > 0) {
        toast.warning(
          t("install.plugin.installSuccess", {
            name: plugin.name,
            count: result.installed,
          }) +
            ` (${result.skipped} skipped, ${result.failed.length} failed)`,
          { id: toastId },
        );
      } else {
        toast.success(
          t("install.plugin.installSuccess", {
            name: plugin.name,
            count: result.installed,
          }),
          { id: toastId },
        );
      }
    } catch (e) {
      toast.error(
        t("install.plugin.installFailed", {
          name: plugin.name,
          message: getErrorMessage(e, t("common.error")),
        }),
        { id: toastId },
      );
    } finally {
      setInstallingPlugin(null);
    }
  };

  const handleRefreshMarket = async (marketId: string, marketName: string) => {
    setRefreshingMarketId(marketId);
    const toastId = toast.loading(t("install.plugin.market.refreshing"));
    try {
      await api.refreshPluginMarket(marketId);
      await loadMarkets();
      await loadPlugins();
      toast.success(
        t("install.plugin.market.refreshSuccess", { name: marketName }),
        { id: toastId },
      );
    } catch (e) {
      toast.error(
        t("install.plugin.market.refreshFailed", {
          message: getErrorMessage(e, t("common.error")),
        }),
        { id: toastId },
      );
    } finally {
      setRefreshingMarketId(null);
    }
  };

  const handleRemoveMarket = async (market: PluginMarketRecord) => {
    if (!window.confirm(t("install.plugin.market.removeConfirm", { name: market.name }))) return;
    try {
      await api.removePluginMarket(market.id);
      await loadMarkets();
      await loadPlugins();
      toast.success(t("install.plugin.market.removeSuccess"));
    } catch (e) {
      toast.error(getErrorMessage(e, t("common.error")));
    }
  };

  const formatTime = (ts: number | null) => {
    if (!ts) return "-";
    const d = new Date(ts);
    return d.toLocaleDateString() + " " + d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  };

  const groupedInstalled = installedSkills.reduce<
    Record<string, { plugin: string; market: string; skills: PluginInstalledSkillDto[] }>
  >((acc, s) => {
    const key = `${s.market_name}/${s.plugin_name}`;
    if (!acc[key]) {
      acc[key] = { plugin: s.plugin_name, market: s.market_name, skills: [] };
    }
    acc[key].skills.push(s);
    return acc;
  }, {});

  return (
    <div className="animate-in fade-in duration-300">
      <div className="app-panel mb-3 p-3.5">
        <div className="flex flex-col gap-3">
          <div className="flex items-center gap-3">
            <div className="app-segmented shrink-0 bg-background">
              {([
                { id: "discover" as const, label: t("install.plugin.discover"), icon: Search },
                { id: "installed" as const, label: t("install.plugin.installed"), icon: HardDrive },
                { id: "markets" as const, label: t("install.plugin.markets"), icon: Store },
              ] as const).map((tab) => {
                const Icon = tab.icon;
                const isActive = subTab === tab.id;
                return (
                  <button
                    key={tab.id}
                    onClick={() => setSubTab(tab.id)}
                    className={cn(
                      "app-segmented-button flex items-center gap-1.5",
                      isActive && "app-segmented-button-active",
                    )}
                  >
                    <Icon className="h-3 w-3" />
                    {tab.label}
                  </button>
                );
              })}
            </div>
          </div>
        </div>
      </div>

      {subTab === "discover" && (
        <div className="pb-8">
          <div className="mb-3 flex items-center gap-3">
            <div className="relative flex-1">
              <Search className="pointer-events-none absolute left-3 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted" />
              <input
                type="text"
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                placeholder={t("install.plugin.searchPlaceholder")}
                className="app-input w-full bg-background pl-9"
                autoCapitalize="none"
                autoCorrect="off"
                spellCheck={false}
              />
            </div>
            <span className="shrink-0 text-[13px] text-muted">
              {t("install.plugin.availableCount", { count: plugins.length })}
            </span>
          </div>

          {pluginsLoading ? (
            <div className="flex items-center justify-center py-16">
              <Loader2 className="h-5 w-5 animate-spin text-muted" />
            </div>
          ) : plugins.length === 0 ? (
            <div className="app-panel flex flex-col items-center justify-center rounded-2xl px-6 py-14 text-center">
              <div className="flex h-12 w-12 items-center justify-center rounded-2xl border border-border bg-background text-muted">
                <Package className="h-5 w-5" />
              </div>
              <h3 className="mt-4 text-[14px] font-semibold text-secondary">
                {markets.length === 0
                  ? t("install.plugin.noMarkets")
                  : t("install.plugin.noPlugins")}
              </h3>
              <p className="mt-1 max-w-md text-[13px] text-muted">
                {markets.length === 0
                  ? t("install.plugin.noMarketsHint")
                  : t("install.plugin.noPluginsHint")}
              </p>
              {markets.length === 0 && (
                <button
                  onClick={() => setAddMarketOpen(true)}
                  className="app-button-primary mt-4"
                >
                  <Plus className="h-3.5 w-3.5" />
                  {t("install.plugin.addFirstMarket")}
                </button>
              )}
            </div>
          ) : (
            <div className="grid grid-cols-2 gap-2.5 lg:grid-cols-3">
              {plugins.map((plugin) => {
                const isInstalled = installedPluginNames.has(plugin.name);
                const isInstalling = installingPlugin === plugin.name;

                return (
                  <div
                    key={plugin.id}
                    className="app-panel flex flex-col gap-2 p-3 transition-colors hover:border-border"
                  >
                    <div className="flex items-start justify-between gap-2">
                      <div
                        className="flex min-w-0 flex-1 items-center gap-2 cursor-pointer"
                        onClick={() => setDetailPlugin(plugin)}
                      >
                        <div className="flex h-6 w-6 shrink-0 items-center justify-center rounded-full border border-border-subtle bg-accent-bg text-[11px] font-bold text-accent-light uppercase">
                          {plugin.name.charAt(0)}
                        </div>
                        <div className="min-w-0">
                          <h3 className="truncate text-[13px] font-semibold text-secondary">
                            {plugin.name}
                          </h3>
                          {plugin.version && (
                            <span className="text-[11px] text-muted">v{plugin.version}</span>
                          )}
                        </div>
                      </div>

                      <div className="flex shrink-0 items-center gap-1">
                        {isInstalled ? (
                          <span
                            className="rounded-[5px] border border-emerald-500/20 bg-emerald-500/10 p-1 text-emerald-400"
                            title={t("install.plugin.installedBadge")}
                          >
                            <Check className="h-3.5 w-3.5" />
                          </span>
                        ) : isInstalling ? (
                          <button
                            disabled
                            className="rounded-[5px] border border-accent-border bg-accent-dark p-1 text-white opacity-70"
                          >
                            <Loader2 className="h-3.5 w-3.5 animate-spin" />
                          </button>
                        ) : (
                          <button
                            onClick={() => handleInstallPlugin(plugin)}
                            className="rounded-[5px] border border-accent-border bg-accent-dark p-1 text-white transition-colors hover:bg-accent"
                            title={t("install.plugin.install")}
                          >
                            <Plus className="h-3.5 w-3.5" />
                          </button>
                        )}
                      </div>
                    </div>

                    <p className="line-clamp-2 text-[13px] leading-4 text-muted">
                      {plugin.description || "-"}
                    </p>

                    <div className="flex flex-wrap items-center gap-1">
                      <span className="rounded-[5px] bg-accent-bg px-1.5 py-0.5 text-[13px] leading-4 font-medium text-accent-light">
                        {plugin.market_name}
                      </span>
                      <span className="inline-flex items-center gap-1 rounded-[5px] border border-border-subtle bg-background px-1.5 py-0.5 text-[13px] leading-4 text-muted">
                        <Layers className="h-3 w-3" />
                        {plugin.skill_names.length}
                      </span>
                      {isInstalled && (
                        <span className="inline-flex items-center gap-1 rounded-[5px] border border-emerald-500/20 bg-emerald-500/10 px-1.5 py-0.5 text-[13px] leading-4 font-medium text-emerald-400">
                          <Check className="h-3 w-3" />
                          {t("install.plugin.installedBadge")}
                        </span>
                      )}
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </div>
      )}

      {subTab === "installed" && (
        <div className="pb-8">
          {installedLoading ? (
            <div className="flex items-center justify-center py-16">
              <Loader2 className="h-5 w-5 animate-spin text-muted" />
            </div>
          ) : installedSkills.length === 0 ? (
            <div className="app-panel flex flex-col items-center justify-center rounded-2xl px-6 py-14 text-center">
              <div className="flex h-12 w-12 items-center justify-center rounded-2xl border border-border bg-background text-muted">
                <HardDrive className="h-5 w-5" />
              </div>
              <h3 className="mt-4 text-[14px] font-semibold text-secondary">
                {t("install.plugin.installedTab.empty")}
              </h3>
              <p className="mt-1 max-w-md text-[13px] text-muted">
                {t("install.plugin.installedTab.emptyHint")}
              </p>
            </div>
          ) : (
            <div className="space-y-4">
              {Object.entries(groupedInstalled).map(([key, group]) => (
                <div key={key}>
                  <div className="mb-2 flex items-center gap-2">
                    <Package className="h-4 w-4 text-accent-light" />
                    <span className="text-[13px] font-semibold text-secondary">
                      {t("install.plugin.installedTab.fromPlugin", { plugin: group.plugin })}
                    </span>
                    <span className="text-[12px] text-muted">
                      {t("install.plugin.installedTab.fromMarket", { market: group.market })}
                    </span>
                  </div>
                  <div className="grid grid-cols-2 gap-2 lg:grid-cols-3">
                    {group.skills.map((skill) => (
                      <div
                        key={skill.skill_id}
                        className="app-panel flex flex-col gap-1 p-3 transition-colors hover:border-border"
                      >
                        <h4 className="truncate text-[13px] font-semibold text-secondary">
                          {skill.skill_name}
                        </h4>
                        <p className="line-clamp-2 text-[12px] text-muted">
                          {skill.skill_description || "-"}
                        </p>
                      </div>
                    ))}
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {subTab === "markets" && (
        <div className="pb-8">
          <div className="mb-3 flex items-center justify-between">
            <h2 className="text-[14px] font-semibold text-secondary">
              {t("install.plugin.markets")}
            </h2>
            <button
              onClick={() => setAddMarketOpen(true)}
              className="app-button-primary"
            >
              <Plus className="h-3.5 w-3.5" />
              {t("install.plugin.market.addMarket")}
            </button>
          </div>

          {marketsLoading ? (
            <div className="flex items-center justify-center py-16">
              <Loader2 className="h-5 w-5 animate-spin text-muted" />
            </div>
          ) : markets.length === 0 ? (
            <div className="app-panel flex flex-col items-center justify-center rounded-2xl px-6 py-14 text-center">
              <div className="flex h-12 w-12 items-center justify-center rounded-2xl border border-border bg-background text-muted">
                <Store className="h-5 w-5" />
              </div>
              <h3 className="mt-4 text-[14px] font-semibold text-secondary">
                {t("install.plugin.market.emptyList")}
              </h3>
              <p className="mt-1 max-w-md text-[13px] text-muted">
                {t("install.plugin.market.emptyListHint")}
              </p>
            </div>
          ) : (
            <div className="space-y-2.5">
              {markets.map((market) => {
                const isRefreshing = refreshingMarketId === market.id;
                return (
                  <div
                    key={market.id}
                    className="app-panel flex flex-col gap-2 p-4 transition-colors hover:border-border"
                  >
                    <div className="flex items-start justify-between gap-3">
                      <div className="flex min-w-0 flex-1 items-center gap-3">
                        <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-border-subtle bg-accent-bg text-[12px] font-bold text-accent-light uppercase">
                          {market.name.charAt(0)}
                        </div>
                        <div className="min-w-0">
                          <h3 className="truncate text-[14px] font-semibold text-secondary">
                            {market.name}
                          </h3>
                          <button
                            onClick={() => {
                              const url = market.url.startsWith("http")
                                ? market.url
                                : `https://github.com/${market.url}`;
                              window.open(url, "_blank");
                            }}
                            className="inline-flex items-center gap-1 text-[12px] text-muted hover:text-accent-light transition-colors"
                          >
                            <Globe className="h-3 w-3" />
                            {market.url}
                            <ExternalLink className="h-2.5 w-2.5" />
                          </button>
                        </div>
                      </div>

                      <div className="flex shrink-0 items-center gap-1">
                        <button
                          onClick={() => handleRefreshMarket(market.id, market.name)}
                          disabled={isRefreshing}
                          className="rounded-[5px] p-1.5 text-muted transition-colors hover:bg-surface-hover hover:text-secondary"
                          title={t("install.plugin.market.refresh")}
                        >
                          <RefreshCw
                            className={cn("h-3.5 w-3.5", isRefreshing && "animate-spin")}
                          />
                        </button>
                        <button
                          onClick={() => handleRemoveMarket(market)}
                          className="rounded-[5px] p-1.5 text-muted transition-colors hover:bg-red-500/10 hover:text-red-400"
                          title={t("common.delete")}
                        >
                          <Trash2 className="h-3.5 w-3.5" />
                        </button>
                      </div>
                    </div>

                    {market.description && (
                      <p className="text-[13px] text-muted">{market.description}</p>
                    )}

                    <div className="flex flex-wrap items-center gap-2">
                      <span className="inline-flex items-center gap-1 rounded-[5px] bg-accent-bg px-1.5 py-0.5 text-[13px] leading-4 font-medium text-accent-light">
                        <Package className="h-3 w-3" />
                        {t("install.plugin.market.pluginCount", { count: market.plugin_count })}
                      </span>
                      {market.last_fetched_at && (
                        <span className="text-[12px] text-muted">
                          {t("install.plugin.market.updatedAt", {
                            time: formatTime(market.last_fetched_at),
                          })}
                        </span>
                      )}
                      {market.last_error && (
                        <span className="text-[12px] text-red-400">
                          {market.last_error}
                        </span>
                      )}
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </div>
      )}

      {detailPlugin && (
        <PluginDetailModal
          plugin={detailPlugin}
          isInstalled={installedPluginNames.has(detailPlugin.name)}
          isInstalling={installingPlugin === detailPlugin.name}
          onClose={() => setDetailPlugin(null)}
          onInstall={() => {
            handleInstallPlugin(detailPlugin);
            setDetailPlugin(null);
          }}
        />
      )}

      {addMarketOpen && (
        <AddMarketDialog
          open={addMarketOpen}
          onClose={() => setAddMarketOpen(false)}
          onAdded={async () => {
            await loadMarkets();
            await loadPlugins();
          }}
        />
      )}
    </div>
  );
}

function PluginDetailModal({
  plugin,
  isInstalled,
  isInstalling,
  onClose,
  onInstall,
}: {
  plugin: PluginWithMarketDto;
  isInstalled: boolean;
  isInstalling: boolean;
  onClose: () => void;
  onInstall: () => void;
}) {
  const { t } = useTranslation();

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      <div className="absolute inset-0 bg-black/70 backdrop-blur-sm" onClick={onClose} />
      <div className="relative bg-surface border border-border rounded-xl w-full max-w-[420px] p-5 shadow-2xl">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-[13px] font-semibold text-primary">
            {t("install.plugin.detail.title")}
          </h2>
          <button
            onClick={onClose}
            className="text-muted hover:text-secondary p-1 rounded transition-colors outline-none"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        <div className="space-y-3">
          <div className="flex items-center gap-3">
            <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg border border-border-subtle bg-accent-bg text-[14px] font-bold text-accent-light uppercase">
              {plugin.name.charAt(0)}
            </div>
            <div>
              <h3 className="text-[14px] font-semibold text-secondary">{plugin.name}</h3>
              {plugin.version && (
                <span className="rounded-[5px] bg-accent-bg px-1.5 py-0.5 text-[12px] font-medium text-accent-light">
                  v{plugin.version}
                </span>
              )}
            </div>
          </div>

          {plugin.description && (
            <p className="text-[13px] text-muted">{plugin.description}</p>
          )}

          <div className="flex items-center gap-2 text-[13px]">
            <span className="text-muted">{t("install.plugin.detail.source")}:</span>
            <span className="font-medium text-accent-light">{plugin.market_name}</span>
          </div>

          <div className="border-t border-border-subtle pt-3">
            <div className="mb-2 text-[13px] font-medium text-secondary">
              {t("install.plugin.detail.skillCount", { count: plugin.skill_names.length })}
            </div>
            <div className="max-h-40 space-y-1 overflow-y-auto scrollbar-hide">
              {plugin.skill_names.map((name) => (
                <div
                  key={name}
                  className="flex items-center gap-2 rounded-md border border-border-subtle bg-background px-2.5 py-1.5 text-[13px] text-secondary"
                >
                  <Layers className="h-3 w-3 text-muted" />
                  {name}
                </div>
              ))}
            </div>
          </div>
        </div>

        <div className="mt-4 flex justify-end gap-2">
          <button
            type="button"
            onClick={onClose}
            className="px-3 py-1.5 text-[13px] font-medium text-muted hover:text-secondary transition-colors"
          >
            {t("common.cancel")}
          </button>
          {isInstalled ? (
            <span className="inline-flex items-center gap-1.5 rounded-lg border border-emerald-500/20 bg-emerald-500/10 px-3 py-1.5 text-[13px] font-medium text-emerald-400">
              <Check className="h-3.5 w-3.5" />
              {t("install.plugin.installedBadge")}
            </span>
          ) : (
            <button
              type="button"
              onClick={onInstall}
              disabled={isInstalling}
              className="app-button-primary"
            >
              {isInstalling ? (
                <Loader2 className="h-3.5 w-3.5 animate-spin" />
              ) : (
                <DownloadCloud className="h-3.5 w-3.5" />
              )}
              {isInstalling
                ? t("install.plugin.installing")
                : t("install.plugin.install")}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

function AddMarketDialog({
  open,
  onClose,
  onAdded,
}: {
  open: boolean;
  onClose: () => void;
  onAdded: () => Promise<void>;
}) {
  const { t } = useTranslation();
  const [url, setUrl] = useState("");
  const [adding, setAdding] = useState(false);

  useEffect(() => {
    if (!open) return;
    setUrl("");
    setAdding(false);
  }, [open]);

  if (!open) return null;

  const handleAdd = async () => {
    if (!url.trim()) return;
    setAdding(true);
    const toastId = toast.loading(t("install.plugin.market.adding"));
    try {
      const market = await api.addPluginMarket(url.trim());
      await onAdded();
      toast.success(
        t("install.plugin.market.addSuccess", { name: market.name }),
        { id: toastId },
      );
      onClose();
    } catch (e) {
      toast.error(
        t("install.plugin.market.addFailed", {
          message: getErrorMessage(e, t("common.error")),
        }),
        { id: toastId },
      );
    } finally {
      setAdding(false);
    }
  };

  const inputClass =
    "w-full bg-background border border-border-subtle rounded-[4px] px-3 py-2 text-[13px] text-secondary focus:outline-none focus:border-border transition-all placeholder-faint";

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      <div className="absolute inset-0 bg-black/70 backdrop-blur-sm" onClick={onClose} />
      <div className="relative bg-surface border border-border rounded-xl w-full max-w-[480px] p-5 shadow-2xl">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-[13px] font-semibold text-primary">
            {t("install.plugin.market.addMarketTitle")}
          </h2>
          <button
            onClick={onClose}
            className="text-muted hover:text-secondary p-1 rounded transition-colors outline-none"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        <p className="mb-3 text-[13px] text-muted">
          {t("install.plugin.market.addMarketDesc")}
        </p>

        <div className="space-y-3">
          <div>
            <input
              type="text"
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && !adding && url.trim()) handleAdd();
              }}
              placeholder={t("install.plugin.market.urlPlaceholder")}
              disabled={adding}
              className={inputClass}
              autoFocus
              autoCapitalize="none"
              autoCorrect="off"
              spellCheck={false}
            />
            <p className="mt-1.5 text-[12px] text-muted">
              {t("install.plugin.market.urlExamples")}
            </p>
          </div>

          <div className="flex justify-end gap-2 pt-1">
            <button
              type="button"
              onClick={onClose}
              disabled={adding}
              className="px-3 py-1.5 text-[13px] font-medium text-muted hover:text-secondary transition-colors"
            >
              {t("common.cancel")}
            </button>
            <button
              type="button"
              onClick={handleAdd}
              disabled={!url.trim() || adding}
              className="app-button-primary"
            >
              {adding ? (
                <Loader2 className="h-3.5 w-3.5 animate-spin" />
              ) : (
                <Plus className="h-3.5 w-3.5" />
              )}
              {adding
                ? t("install.plugin.market.adding")
                : t("install.plugin.market.addMarket")}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
