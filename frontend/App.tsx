import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import Header from "./components/Header";
import TabButton from "./components/TabButton";
import InstallTab from "./components/InstallTab";
import SyncTab from "./components/SyncTab";

interface AppState {
  textures_path: string | null;
  initial_setup_done: boolean;
  last_sync_commit: string | null;
}

type Tab = "install" | "sync";

function App() {
  const [texturesDir, setTexturesDir] = useState("");
  const [gitAvailable, setGitAvailable] = useState<boolean | null>(null);
  const [gitError, setGitError] = useState("");
  const [activeTab, setActiveTab] = useState<Tab>("install");
  const [initialSetupDone, setInitialSetupDone] = useState(false);
  const [lastSyncCommit, setLastSyncCommit] = useState<string | null>(null);
  const [stateLoaded, setStateLoaded] = useState(false);

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

        // If setup is done, default to sync tab
        if (state.initial_setup_done) {
          setActiveTab("sync");
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
    } catch (e) {
      console.error("Failed to mark setup complete:", e);
    }
  };

  // Handle sync complete
  const handleSyncComplete = async (commitSha: string) => {
    try {
      await invoke("update_last_sync_commit", { commitSha });
      setLastSyncCommit(commitSha);
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

  if (!stateLoaded) {
    return (
      <div className="min-h-screen bg-zinc-900 text-zinc-100 flex items-center justify-center">
        <div className="animate-spin h-8 w-8 border-2 border-zinc-600 border-t-blue-400 rounded-full" />
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-zinc-900 text-zinc-100 p-6 overflow-auto">
      <div className="max-w-xl mx-auto space-y-6">
        <Header />

        {/* Tabs */}
        <div className="flex gap-1 border-b border-zinc-700">
          <TabButton
            label="Install"
            isActive={activeTab === "install"}
            onClick={() => setActiveTab("install")}
          />
          <TabButton
            label="Sync"
            isActive={activeTab === "sync"}
            onClick={() => setActiveTab("sync")}
            disabled={!initialSetupDone}
          />
        </div>

        {/* Tab content */}
        <section className="bg-zinc-800 rounded-lg p-5 border border-zinc-700">
          {activeTab === "install" ? (
            <>
              <h2 className="text-lg font-semibold text-zinc-100 mb-4 uppercase tracking-wide">
                First Time Installation
              </h2>
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
                onSyncComplete={handleSyncComplete}
              />
            </>
          )}
        </section>
      </div>
    </div>
  );
}

export default App;
