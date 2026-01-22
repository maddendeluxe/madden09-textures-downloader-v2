import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import DirectoryPicker from "./DirectoryPicker";
import InstallButton from "./InstallButton";
import ProgressDisplay from "./ProgressDisplay";
import ExistingFolderDialog from "./ExistingFolderDialog";
import { TARGET_FOLDER } from "../config";

interface ProgressPayload {
  stage: string;
  message: string;
  percent: number | null;
}

type InstallStatus = "idle" | "installing" | "complete" | "error";

interface InstallTabProps {
  texturesDir: string;
  setTexturesDir: (dir: string) => void;
  gitAvailable: boolean | null;
  gitError: string;
  onInstallComplete: (commitSha: string) => void;
}

function InstallTab({
  texturesDir,
  setTexturesDir,
  gitAvailable,
  gitError,
  onInstallComplete,
}: InstallTabProps) {
  const [showFolderDialog, setShowFolderDialog] = useState(false);
  const [installStatus, setInstallStatus] = useState<InstallStatus>("idle");
  const [progressMessages, setProgressMessages] = useState<string[]>([]);
  const [progressPercent, setProgressPercent] = useState<number | null>(null);
  const [currentStage, setCurrentStage] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  // Listen for progress events
  useEffect(() => {
    const unlisten = listen<ProgressPayload>("install-progress", (event) => {
      const { stage, message, percent } = event.payload;

      setCurrentStage(stage);
      setProgressMessages((prev) => [...prev, message]);
      if (percent !== null) {
        setProgressPercent(percent);
      }

      if (stage === "complete") {
        setInstallStatus("complete");
        // Get the latest commit SHA and notify parent
        invoke<string>("get_latest_commit")
          .then((sha) => onInstallComplete(sha))
          .catch(console.error);
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [onInstallComplete]);

  const handleStartInstall = async () => {
    if (!texturesDir) {
      setErrorMessage("Please select a textures directory first");
      return;
    }

    try {
      const exists = await invoke<boolean>("check_existing_folder", {
        texturesDir,
      });

      if (exists) {
        setShowFolderDialog(true);
        return;
      }

      await startInstallation();
    } catch (e) {
      setErrorMessage(`Error: ${e}`);
    }
  };

  const startInstallation = async () => {
    setInstallStatus("installing");
    setProgressMessages([]);
    setProgressPercent(0);
    setCurrentStage(null);
    setErrorMessage(null);

    try {
      await invoke("start_installation", { texturesDir });
    } catch (e) {
      setInstallStatus("error");
      setErrorMessage(`Installation failed: ${e}`);
    }
  };

  const handleBackup = async () => {
    setShowFolderDialog(false);
    try {
      const backupName = await invoke<string>("backup_existing_folder", {
        texturesDir,
      });
      setProgressMessages([`Backed up existing folder to: ${backupName}`]);
      await startInstallation();
    } catch (e) {
      setErrorMessage(`Backup failed: ${e}`);
    }
  };

  const handleDelete = async () => {
    setShowFolderDialog(false);
    try {
      await invoke("delete_existing_folder", { texturesDir });
      setProgressMessages(["Deleted existing folder"]);
      await startInstallation();
    } catch (e) {
      setErrorMessage(`Delete failed: ${e}`);
    }
  };

  const handleCancel = () => {
    setShowFolderDialog(false);
  };

  const isInstalling = installStatus === "installing";

  return (
    <>
      <div className="space-y-4">
        {/* Git availability warning */}
        {gitAvailable === false && (
          <div className="bg-red-900/50 border border-red-700 rounded-lg p-4">
            <p className="text-red-200 font-medium">Git not available</p>
            <p className="text-red-300 text-sm mt-1">{gitError}</p>
          </div>
        )}

        <DirectoryPicker
          value={texturesDir}
          onChange={setTexturesDir}
          disabled={isInstalling}
        />

        <InstallButton
          onClick={handleStartInstall}
          disabled={!texturesDir || isInstalling || gitAvailable === false}
          isInstalling={isInstalling}
        />

        {/* Error message */}
        {errorMessage && (
          <div className="p-3 bg-red-900/30 border border-red-800 rounded text-red-300 text-sm">
            {errorMessage}
          </div>
        )}

        {/* Progress display */}
        {(isInstalling || installStatus === "complete") && (
          <ProgressDisplay
            messages={progressMessages}
            percent={progressPercent}
            stage={currentStage}
            isComplete={installStatus === "complete"}
          />
        )}
      </div>

      {/* Existing folder dialog */}
      {showFolderDialog && (
        <ExistingFolderDialog
          folderName={TARGET_FOLDER}
          onBackup={handleBackup}
          onDelete={handleDelete}
          onCancel={handleCancel}
        />
      )}
    </>
  );
}

export default InstallTab;
