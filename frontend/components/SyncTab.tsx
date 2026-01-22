import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import SyncProgress from "./SyncProgress";

interface SyncStatusResult {
  latest_commit_sha: string;
  files_to_download: number;
  files_to_delete: number;
  files_up_to_date: number;
  is_up_to_date: boolean;
}

interface SyncResult {
  files_downloaded: number;
  files_deleted: number;
  files_skipped: number;
  new_commit_sha: string;
}

interface SyncProgressPayload {
  stage: string;
  message: string;
  current: number | null;
  total: number | null;
}

type SyncStatus = "idle" | "checking" | "syncing" | "complete" | "error";

interface SyncTabProps {
  texturesDir: string;
  lastSyncCommit: string | null;
  onSyncComplete: (commitSha: string) => void;
}

function SyncTab({ texturesDir, lastSyncCommit, onSyncComplete }: SyncTabProps) {
  const [syncStatus, setSyncStatus] = useState<SyncStatus>("idle");
  const [statusResult, setStatusResult] = useState<SyncStatusResult | null>(null);
  const [syncResult, setSyncResult] = useState<SyncResult | null>(null);
  const [progressMessages, setProgressMessages] = useState<SyncProgressPayload[]>([]);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  // Listen for sync progress events
  useEffect(() => {
    const unlisten = listen<SyncProgressPayload>("sync-progress", (event) => {
      setProgressMessages((prev) => [...prev, event.payload]);

      if (event.payload.stage === "complete") {
        setSyncStatus("complete");
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Check status when tab is opened or texturesDir changes
  useEffect(() => {
    if (texturesDir) {
      checkSyncStatus();
    }
  }, [texturesDir]);

  const checkSyncStatus = async () => {
    setSyncStatus("checking");
    setErrorMessage(null);

    try {
      const result = await invoke<SyncStatusResult>("check_sync_status", {
        texturesDir,
      });
      setStatusResult(result);
      setSyncStatus("idle");
    } catch (e) {
      setErrorMessage(`Failed to check status: ${e}`);
      setSyncStatus("error");
    }
  };

  const handleRunSync = async () => {
    setSyncStatus("syncing");
    setProgressMessages([]);
    setSyncResult(null);
    setErrorMessage(null);

    try {
      const result = await invoke<SyncResult>("run_sync", {
        texturesDir,
      });
      setSyncResult(result);
      onSyncComplete(result.new_commit_sha);
      // Refresh status after sync
      await checkSyncStatus();
    } catch (e) {
      setErrorMessage(`Sync failed: ${e}`);
      setSyncStatus("error");
    }
  };

  const isSyncing = syncStatus === "syncing";
  const isChecking = syncStatus === "checking";

  return (
    <div className="space-y-4">
      {/* Current status info */}
      <div className="bg-zinc-900 border border-zinc-700 rounded-lg p-4 space-y-3">
        <div className="flex items-center justify-between">
          <h3 className="text-sm font-medium text-zinc-300">Sync Status</h3>
          <button
            onClick={checkSyncStatus}
            disabled={isChecking || isSyncing}
            className="text-xs text-blue-400 hover:text-blue-300 disabled:opacity-50"
          >
            {isChecking ? "Checking..." : "Refresh"}
          </button>
        </div>

        {statusResult && (
          <div className="space-y-2 text-sm">
            <div className="flex justify-between">
              <span className="text-zinc-400">Latest commit:</span>
              <span className="text-zinc-300 font-mono text-xs">
                {statusResult.latest_commit_sha.slice(0, 7)}
              </span>
            </div>
            {lastSyncCommit && (
              <div className="flex justify-between">
                <span className="text-zinc-400">Last synced:</span>
                <span className="text-zinc-300 font-mono text-xs">
                  {lastSyncCommit.slice(0, 7)}
                </span>
              </div>
            )}
            <div className="border-t border-zinc-700 pt-2 mt-2">
              {statusResult.is_up_to_date ? (
                <div className="flex items-center gap-2 text-green-400">
                  <svg className="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
                  </svg>
                  <span>Textures are up to date!</span>
                </div>
              ) : (
                <div className="space-y-1">
                  <div className="flex justify-between">
                    <span className="text-zinc-400">Files to download:</span>
                    <span className={statusResult.files_to_download > 0 ? "text-yellow-400" : "text-zinc-300"}>
                      {statusResult.files_to_download}
                    </span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-zinc-400">Files to delete:</span>
                    <span className={statusResult.files_to_delete > 0 ? "text-red-400" : "text-zinc-300"}>
                      {statusResult.files_to_delete}
                    </span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-zinc-400">Files up to date:</span>
                    <span className="text-zinc-300">{statusResult.files_up_to_date}</span>
                  </div>
                </div>
              )}
            </div>
          </div>
        )}

        {isChecking && !statusResult && (
          <div className="flex items-center gap-2 text-zinc-400">
            <svg className="animate-spin h-4 w-4" fill="none" viewBox="0 0 24 24">
              <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
              <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
            </svg>
            <span className="text-sm">Checking for updates...</span>
          </div>
        )}
      </div>

      {/* Sync button */}
      <button
        onClick={handleRunSync}
        disabled={!texturesDir || isSyncing || isChecking || statusResult?.is_up_to_date}
        className={`
          w-full py-3 rounded-lg font-medium transition-all
          ${statusResult?.is_up_to_date
            ? "bg-zinc-700 text-zinc-500 cursor-not-allowed"
            : isSyncing || isChecking
            ? "bg-zinc-700 text-zinc-400 cursor-wait"
            : "bg-blue-600 hover:bg-blue-500 text-white"
          }
        `}
      >
        {isSyncing ? "Syncing..." : statusResult?.is_up_to_date ? "Up to Date" : "Run Sync"}
      </button>

      {/* Error message */}
      {errorMessage && (
        <div className="p-3 bg-red-900/30 border border-red-800 rounded text-red-300 text-sm">
          {errorMessage}
        </div>
      )}

      {/* Progress display */}
      {(isSyncing || syncStatus === "complete") && progressMessages.length > 0 && (
        <SyncProgress
          messages={progressMessages}
          isComplete={syncStatus === "complete"}
          result={syncResult}
        />
      )}

      {/* Info about disabled textures */}
      <div className="bg-zinc-900/50 border border-zinc-700 rounded-lg p-3 text-xs text-zinc-500">
        <p className="font-medium text-zinc-400 mb-1">About Sync</p>
        <ul className="space-y-1 list-disc list-inside">
          <li>Files in <code className="text-zinc-400">user-customs/</code> are never modified</li>
          <li>Disabled textures (dash-prefixed) are kept disabled but updated</li>
          <li>New textures from the repo are downloaded automatically</li>
          <li>Removed textures are deleted (unless you have a disabled version)</li>
        </ul>
      </div>
    </div>
  );
}

export default SyncTab;
