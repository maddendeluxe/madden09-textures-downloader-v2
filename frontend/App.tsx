import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import Header from "./components/Header";
import TabButton from "./components/TabButton";
import InstallTab from "./components/InstallTab";
import SyncTab from "./components/SyncTab";
import SyncDisclaimerDialog from "./components/SyncDisclaimerDialog";
import AppOutdatedModal from "./components/AppOutdatedModal";
import FetchErrorModal from "./components/FetchErrorModal";

interface AppState {
  textures_path: string | null;
  initial_setup_done: boolean;
  last_sync_commit: string | null;
  last_sync_timestamp: string | null;
  github_token: string | null;
  sync_disclaimer_acknowledged: boolean;
}

interface InstallerData {
  min_download_app_version: string;
  total_size: string;
  downloader_app_url: string;
}

interface InstallerDataResult {
  data: InstallerData | null;
  error: string | null;
}

type Tab = "install" | "sync";

function App() {
  const [texturesDir, setTexturesDir] = useState("");
  const [gitAvailable, setGitAvailable] = useState<boolean | null>(null);
  const [gitError, setGitError] = useState("");
  const [activeTab, setActiveTab] = useState<Tab>("install");
  const [initialSetupDone, setInitialSetupDone] = useState(false);
  const [lastSyncCommit, setLastSyncCommit] = useState<string | null>(null);
  const [lastSyncTimestamp, setLastSyncTimestamp] = useState<string | null>(null);
  const [githubToken, setGithubToken] = useState<string | null>(null);
  const [syncDisclaimerAcknowledged, setSyncDisclaimerAcknowledged] = useState(false);
  const [showSyncDisclaimer, setShowSyncDisclaimer] = useState(false);
  const [stateLoaded, setStateLoaded] = useState(false);

  // App version and installer data
  const [appVersion, setAppVersion] = useState<string | null>(null);
  const [installerData, setInstallerData] = useState<InstallerData | null>(null);
  const [installerDataError, setInstallerDataError] = useState<string | null>(null);
  const [isAppOutdated, setIsAppOutdated] = useState(false);
  const [requiredVersion, setRequiredVersion] = useState<string>("");

  // Load saved state on mount
  useEffect(() => {
    const loadAppState = async () => {
      try {
        const state = await invoke<AppState>("load_state");
        if (state.textures_path) {
          setTexturesDir(state.textures_path);
        }
        setInitialSetupDone(state.initial_setup_done);
        setLastSyncCommit(state.last_sync_commit);
        setLastSyncTimestamp(state.last_sync_timestamp);
        setGithubToken(state.github_token);
        setSyncDisclaimerAcknowledged(state.sync_disclaimer_acknowledged || false);

        // If setup is done, default to sync tab
        if (state.initial_setup_done) {
          setActiveTab("sync");
          // Show disclaimer if not yet acknowledged
          if (!state.sync_disclaimer_acknowledged) {
            setShowSyncDisclaimer(true);
          }
        }
      } catch (e) {
        console.error("Failed to load state:", e);
      }
      setStateLoaded(true);
    };
    loadAppState();
  }, []);

  // Check if git is available on mount
  useEffect(() => {
    const checkGit = async () => {
      try {
        const available = await invoke<boolean>("check_git_installed");
        setGitAvailable(available);
        if (!available) {
          const error = await invoke<string>("get_git_error");
          setGitError(error);
        }
      } catch (e) {
        setGitAvailable(false);
        setGitError("Failed to check git availability");
      }
    };
    checkGit();
  }, []);

  // Fetch app version and installer data on mount
  useEffect(() => {
    const fetchAppInfo = async () => {
      try {
        // Get app version
        const version = await invoke<string>("get_app_version");
        setAppVersion(version);

        // Fetch installer data from repo
        const result = await invoke<InstallerDataResult>("fetch_installer_data");

        if (result.error) {
          setInstallerDataError(result.error);
          return;
        }

        if (result.data) {
          setInstallerData(result.data);

          // Check version compatibility
          const comparison = await invoke<number>("compare_versions", {
            v1: version,
            v2: result.data.min_download_app_version,
          });

          if (comparison < 0) {
            // App is outdated
            setIsAppOutdated(true);
            setRequiredVersion(result.data.min_download_app_version);
          }
        }
      } catch (e) {
        console.error("Failed to fetch app info:", e);
        setInstallerDataError(String(e));
      }
    };
    fetchAppInfo();
  }, []);

  // Retry fetching installer data
  const handleRetryFetch = async () => {
    setInstallerDataError(null);
    try {
      const result = await invoke<InstallerDataResult>("fetch_installer_data");

      if (result.error) {
        setInstallerDataError(result.error);
        return;
      }

      if (result.data) {
        setInstallerData(result.data);

        // Check version compatibility
        if (appVersion) {
          const comparison = await invoke<number>("compare_versions", {
            v1: appVersion,
            v2: result.data.min_download_app_version,
          });

          if (comparison < 0) {
            setIsAppOutdated(true);
            setRequiredVersion(result.data.min_download_app_version);
          }
        }
      }
    } catch (e) {
      console.error("Retry failed:", e);
      setInstallerDataError(String(e));
    }
  };

  // Save textures path when it changes
  const handleTexturesDirChange = async (dir: string) => {
    setTexturesDir(dir);
    try {
      await invoke("set_textures_path", { path: dir });
    } catch (e) {
      console.error("Failed to save textures path:", e);
    }
  };

  // Handle install complete
  const handleInstallComplete = async (commitSha: string) => {
    try {
      await invoke("mark_setup_complete", { commitSha });
      setInitialSetupDone(true);
      setLastSyncCommit(commitSha);
      setLastSyncTimestamp(new Date().toISOString());
    } catch (e) {
      console.error("Failed to mark setup complete:", e);
    }
  };

  // Handle sync complete
  const handleSyncComplete = async (commitSha: string) => {
    try {
      await invoke("update_last_sync_commit", { commitSha });
      setLastSyncCommit(commitSha);
      setLastSyncTimestamp(new Date().toISOString());
    } catch (e) {
      console.error("Failed to update sync commit:", e);
    }
  };

  // Handle manual setup toggle
  const handleSetupToggle = async (done: boolean) => {
    try {
      await invoke("set_initial_setup_done", { done });
      setInitialSetupDone(done);
      if (done && !lastSyncCommit) {
        // If marking as done manually, try to get latest commit
        try {
          const sha = await invoke<string>("get_latest_commit");
          await invoke("update_last_sync_commit", { commitSha: sha });
          setLastSyncCommit(sha);
        } catch {
          // Ignore errors
        }
      }
    } catch (e) {
      console.error("Failed to set setup done:", e);
    }
  };

  // Handle GitHub token change
  const handleTokenChange = async (token: string) => {
    try {
      await invoke("set_github_token", { token });
      setGithubToken(token || null);
    } catch (e) {
      console.error("Failed to save GitHub token:", e);
    }
  };

  // Handle tab change - show disclaimer when entering sync tab for first time
  const handleTabChange = (tab: Tab) => {
    if (tab === "sync" && !syncDisclaimerAcknowledged) {
      setShowSyncDisclaimer(true);
    }
    setActiveTab(tab);
  };

  // Handle disclaimer acknowledgment
  const handleDisclaimerAcknowledge = async (dontShowAgain: boolean) => {
    setShowSyncDisclaimer(false);
    if (dontShowAgain) {
      setSyncDisclaimerAcknowledged(true);
      try {
        await invoke("set_sync_disclaimer_acknowledged", { acknowledged: true });
      } catch (e) {
        console.error("Failed to save disclaimer preference:", e);
      }
    }
  };

  // Show loading while state or installer data is loading
  if (!stateLoaded || (!installerData && !installerDataError)) {
    return (
      <div className="min-h-screen bg-zinc-900 text-zinc-100 flex items-center justify-center">
        <div className="animate-spin h-8 w-8 border-2 border-zinc-600 border-t-blue-400 rounded-full" />
      </div>
    );
  }

  // Show outdated modal (blocking - no dismiss)
  if (isAppOutdated && appVersion) {
    return (
      <div className="min-h-screen bg-zinc-900 text-zinc-100">
        <AppOutdatedModal
          currentVersion={appVersion}
          requiredVersion={requiredVersion}
          downloaderAppUrl={installerData?.downloader_app_url || ""}
        />
      </div>
    );
  }

  // Show fetch error modal (blocking until retry succeeds)
  if (installerDataError && !installerData) {
    return (
      <div className="min-h-screen bg-zinc-900 text-zinc-100">
        <FetchErrorModal
          error={installerDataError}
          onRetry={handleRetryFetch}
        />
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-zinc-900 text-zinc-100 p-6 overflow-auto flex flex-col">
      {/* Sync Disclaimer Dialog */}
      {showSyncDisclaimer && (
        <SyncDisclaimerDialog onAcknowledge={handleDisclaimerAcknowledge} />
      )}

      <div className="max-w-xl mx-auto space-y-6 flex-1">
        <Header version={appVersion || undefined} />

        {/* Tabs */}
        <div className="flex gap-1 border-b border-zinc-700">
          <TabButton
            label="Install"
            isActive={activeTab === "install"}
            onClick={() => handleTabChange("install")}
          />
          <TabButton
            label="Sync"
            isActive={activeTab === "sync"}
            onClick={() => handleTabChange("sync")}
            disabled={!initialSetupDone}
          />
        </div>

        {/* Tab content */}
        <section className="bg-zinc-800 rounded-lg p-5 border border-zinc-700">
          {activeTab === "install" ? (
            <>
              <h2 className="text-lg font-semibold text-zinc-100 mb-1 uppercase tracking-wide">
                First Time Installation
              </h2>
              {installerData?.total_size && (
                <p className="text-sm text-zinc-400 mb-4">
                  Estimated download size: {installerData.total_size} GB
                </p>
              )}
              <InstallTab
                texturesDir={texturesDir}
                setTexturesDir={handleTexturesDirChange}
                gitAvailable={gitAvailable}
                gitError={gitError}
                onInstallComplete={handleInstallComplete}
              />

              {/* Manual setup checkbox */}
              <div className="mt-6 pt-4 border-t border-zinc-700">
                <label className="flex items-center gap-3 text-sm text-zinc-400 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={initialSetupDone}
                    onChange={(e) => handleSetupToggle(e.target.checked)}
                    className="w-4 h-4 rounded bg-zinc-700 border-zinc-600 text-blue-500 focus:ring-blue-500 focus:ring-offset-zinc-800"
                  />
                  <span>
                    I already have textures installed (enable Sync tab)
                  </span>
                </label>
                <p className="mt-2 text-xs text-zinc-500 ml-7">
                  Check this if you installed textures manually or from a previous version.
                  This enables the Sync tab for downloading updates.
                </p>
              </div>
            </>
          ) : (
            <>
              <h2 className="text-lg font-semibold text-zinc-100 mb-4 uppercase tracking-wide">
                Sync Textures
              </h2>
              <SyncTab
                texturesDir={texturesDir}
                lastSyncCommit={lastSyncCommit}
                lastSyncTimestamp={lastSyncTimestamp}
                githubToken={githubToken}
                onSyncComplete={handleSyncComplete}
                onTokenChange={handleTokenChange}
              />
            </>
          )}
        </section>
      </div>

      {/* Footer with version */}
      {appVersion && (
        <footer className="text-center text-xs text-zinc-600 mt-4 pb-2">
          v{appVersion}
        </footer>
      )}
    </div>
  );
}

export default App;
