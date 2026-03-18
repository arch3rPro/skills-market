import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { X, Folder, CheckCircle2, Circle, Loader2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { cn } from "../utils";
import { getSkillDocument, type ManagedSkill, type SkillDocument } from "../lib/tauri";
import { SkillMarkdown } from "./SkillMarkdown";

export interface SyncMeta {
  syncedToolKeys: string[];
  syncedToolLabels: string[];
  pendingToolKeys: string[];
  pendingToolLabels: string[];
}

interface Props {
  skill: ManagedSkill | null;
  onClose: () => void;
  syncMeta?: SyncMeta | null;
  syncing?: boolean;
  onSync?: (mode: "sync" | "unsync") => void;
}

export function SkillDetailPanel({ skill, onClose, syncMeta, syncing, onSync }: Props) {
  const { t } = useTranslation();
  const [doc, setDoc] = useState<SkillDocument | null>(null);
  const [loading, setLoading] = useState(false);
  const requestIdRef = useRef(0);

  useEffect(() => {
    if (!skill) return;
    requestIdRef.current += 1;
    const requestId = requestIdRef.current;

    // Loading state is intentionally toggled when input skill changes.
    // eslint-disable-next-line react-hooks/set-state-in-effect
    setLoading(true);
    getSkillDocument(skill.id)
      .then((nextDoc) => {
        if (requestId === requestIdRef.current) {
          setDoc(nextDoc);
        }
      })
      .catch(() => {
        if (requestId === requestIdRef.current) {
          setDoc(null);
        }
      })
      .finally(() => {
        if (requestId === requestIdRef.current) {
          setLoading(false);
        }
      });
  }, [skill]);

  if (!skill) return null;
  const activeDoc = doc?.skill_id === skill.id ? doc : null;

  return createPortal(
    <div className="fixed inset-y-0 right-0 left-[220px] z-50 flex">
      <div className="absolute inset-0 bg-black/60 backdrop-blur-sm" onClick={onClose} />
      <div className="relative flex h-full min-h-0 w-full flex-col border-l border-border-subtle bg-bg-secondary shadow-2xl animate-in slide-in-from-right duration-200">
        <div className="border-b border-border-subtle px-6 pt-6 pb-5">
          <div className="mb-3 flex items-start justify-between gap-4">
            <h2 className="min-w-0 text-[30px] font-semibold leading-tight tracking-tight text-primary">
              <span className="block truncate">{skill.name}</span>
            </h2>
            <button
              onClick={onClose}
              className="text-muted hover:text-secondary p-1.5 rounded-[4px] hover:bg-surface-hover transition-colors outline-none shrink-0"
            >
              <X className="w-4 h-4" />
            </button>
          </div>
          {skill.description && (
            <p className="text-[15px] leading-7 text-secondary line-clamp-3">{skill.description}</p>
          )}
          <div className="mt-4 flex min-w-0 items-center gap-2 text-[13px] text-muted">
            <Folder className="h-3.5 w-3.5 shrink-0" />
            <span className="font-mono truncate" title={skill.central_path}>
              {skill.central_path}
            </span>
          </div>
        </div>

        {syncMeta && onSync && (
          <div className="flex flex-wrap items-center justify-between gap-2 border-b border-border-subtle px-6 py-3 text-[14px]">
            <div className="flex min-w-0 flex-wrap items-center gap-2 text-muted">
              {syncMeta.syncedToolKeys.length > 0 ? (
                <CheckCircle2 className="h-3.5 w-3.5 text-emerald-500" />
              ) : (
                <Circle className="h-3.5 w-3.5 text-faint" />
              )}
              <span className="rounded-full border border-border-subtle bg-surface px-2.5 py-1 text-[13px] font-medium text-tertiary">
                {t("mySkills.syncSummary", {
                  synced: syncMeta.syncedToolKeys.length,
                  total: syncMeta.syncedToolKeys.length + syncMeta.pendingToolKeys.length,
                })}
              </span>
              {syncMeta.syncedToolLabels.length > 0 && (
                <span className="truncate text-[13.5px] text-muted">{syncMeta.syncedToolLabels.join(", ")}</span>
              )}
            </div>
            <div className="flex items-center gap-2">
              {syncMeta.pendingToolKeys.length > 0 && (
                <button
                  onClick={() => onSync("sync")}
                  disabled={syncing}
                  className={cn(
                    "inline-flex h-8 items-center rounded-lg px-3 text-[13px] font-medium transition-colors outline-none",
                    "text-muted hover:bg-surface-hover hover:text-secondary",
                    syncing && "opacity-50"
                  )}
                >
                  {syncing ? <Loader2 className="h-3 w-3 animate-spin" /> : t("mySkills.syncMissing", { count: syncMeta.pendingToolKeys.length })}
                </button>
              )}
              {syncMeta.syncedToolKeys.length > 0 && (
                <button
                  onClick={() => onSync("unsync")}
                  disabled={syncing}
                  className={cn(
                    "inline-flex h-8 items-center rounded-lg px-3 text-[13px] font-medium transition-colors outline-none",
                    "text-faint hover:bg-red-500/10 hover:text-red-400",
                    syncing && "opacity-50"
                  )}
                >
                  {t("mySkills.unsyncSelected", { count: syncMeta.syncedToolKeys.length })}
                </button>
              )}
            </div>
          </div>
        )}

        <div className="min-h-0 flex-1 overflow-y-auto px-5 py-5 scrollbar-hide">
          {loading ? (
            <div className="text-[13px] text-muted text-center mt-12">{t("common.loading")}</div>
          ) : activeDoc ? (
            <SkillMarkdown content={activeDoc.content} />
          ) : (
            <div className="text-[13px] text-muted text-center mt-12">{t("common.documentMissing")}</div>
          )}
        </div>
      </div>
    </div>,
    document.body
  );
}
