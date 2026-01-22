interface VerificationFile {
  path: string;
  to_disabled: boolean;
}

interface VerificationDialogProps {
  filesToDownload: VerificationFile[];
  filesToDelete: string[];
  onConfirm: () => void;
  onCancel: () => void;
  isApplying: boolean;
}

function VerificationDialog({
  filesToDownload,
  filesToDelete,
  onConfirm,
  onCancel,
  isApplying,
}: VerificationDialogProps) {
  const totalChanges = filesToDownload.length + filesToDelete.length;

  return (
    <div className="fixed inset-0 bg-black/70 flex items-center justify-center z-50 p-4">
      <div className="bg-zinc-800 border border-zinc-600 rounded-lg max-w-2xl w-full max-h-[80vh] flex flex-col">
        <div className="p-4 border-b border-zinc-700">
          <h3 className="text-lg font-semibold text-zinc-100">
            Verification Found Discrepancies
          </h3>
          <p className="text-sm text-zinc-400 mt-1">
            The following {totalChanges} file{totalChanges !== 1 ? "s" : ""} differ from the repository.
            Would you like to fix them?
          </p>
        </div>

        <div className="flex-1 overflow-y-auto p-4 space-y-4">
          {filesToDownload.length > 0 && (
            <div>
              <h4 className="text-sm font-medium text-blue-400 mb-2 flex items-center gap-2">
                <svg className="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" />
                </svg>
                Files to download ({filesToDownload.length})
              </h4>
              <div className="bg-zinc-900 border border-zinc-700 rounded p-2 max-h-40 overflow-y-auto">
                <ul className="text-xs text-zinc-300 space-y-0.5 font-mono">
                  {filesToDownload.map((file, i) => (
                    <li key={i} className="truncate">
                      {file.to_disabled ? (
                        <span className="text-yellow-400">[-] </span>
                      ) : null}
                      {file.path}
                    </li>
                  ))}
                </ul>
              </div>
            </div>
          )}

          {filesToDelete.length > 0 && (
            <div>
              <h4 className="text-sm font-medium text-red-400 mb-2 flex items-center gap-2">
                <svg className="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                </svg>
                Files to delete ({filesToDelete.length})
              </h4>
              <div className="bg-zinc-900 border border-zinc-700 rounded p-2 max-h-40 overflow-y-auto">
                <ul className="text-xs text-zinc-300 space-y-0.5 font-mono">
                  {filesToDelete.map((path, i) => (
                    <li key={i} className="truncate">{path}</li>
                  ))}
                </ul>
              </div>
            </div>
          )}
        </div>

        <div className="p-4 border-t border-zinc-700 flex gap-3 justify-end">
          <button
            onClick={onCancel}
            disabled={isApplying}
            className="px-4 py-2 bg-zinc-700 hover:bg-zinc-600 disabled:bg-zinc-800 disabled:text-zinc-600 text-sm rounded transition-colors"
          >
            Skip
          </button>
          <button
            onClick={onConfirm}
            disabled={isApplying}
            className={`px-4 py-2 text-sm rounded transition-colors ${
              isApplying
                ? "bg-blue-800 text-blue-300 cursor-wait"
                : "bg-blue-600 hover:bg-blue-500 text-white"
            }`}
          >
            {isApplying ? "Applying..." : "Fix Discrepancies"}
          </button>
        </div>
      </div>
    </div>
  );
}

export default VerificationDialog;
