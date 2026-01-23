interface SyncFile {
  path: string;
  to_disabled: boolean;
}

interface SyncWarningDialogProps {
  filesToReplace: SyncFile[];
  filesToDelete: string[];
  onConfirm: () => void;
  onCancel: () => void;
}

function SyncWarningDialog({
  filesToReplace,
  filesToDelete,
  onConfirm,
  onCancel,
}: SyncWarningDialogProps) {
  const hasReplacements = filesToReplace.length > 0;
  const hasDeletions = filesToDelete.length > 0;

  return (
    <div className="fixed inset-0 bg-black/70 flex items-center justify-center z-50 p-4">
      <div className="bg-zinc-800 border border-zinc-600 rounded-lg max-w-2xl w-full max-h-[80vh] flex flex-col">
        {/* Header */}
        <div className="p-4 border-b border-zinc-700">
          <div className="flex items-center gap-2 text-yellow-400">
            <svg className="h-6 w-6 flex-shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
            </svg>
            <h2 className="text-lg font-semibold">Warning: Local Files Will Be Modified</h2>
          </div>
        </div>

        {/* Content */}
        <div className="p-4 overflow-y-auto flex-1 space-y-4">
          {hasReplacements && (
            <div>
              <div className="flex items-center gap-2 mb-2">
                <span className="text-orange-400 font-medium">
                  {filesToReplace.length} file(s) will be REPLACED
                </span>
              </div>
              <p className="text-sm text-zinc-400 mb-2">
                These files exist locally but are different than the mod's files.
                If these are custom textures, be sure to have a backup or copy them to the
                <code className="mx-1 px-1 bg-zinc-700 rounded">user-customs</code>
                folder now before proceeding. The sync will replace them with the mod-default files.
              </p>
              <div className="bg-zinc-900 border border-zinc-700 rounded p-2 max-h-60 overflow-y-auto">
                <ul className="text-xs text-zinc-300 font-mono space-y-0.5">
                  {filesToReplace.map((file) => (
                    <li key={file.path} className="truncate">
                      {file.to_disabled ? `-${file.path}` : file.path}
                    </li>
                  ))}
                </ul>
              </div>
            </div>
          )}

          {hasDeletions && (
            <div>
              <div className="flex items-center gap-2 mb-2">
                <span className="text-red-400 font-medium">
                  {filesToDelete.length} file(s) will be DELETED
                </span>
              </div>
              <p className="text-sm text-zinc-400 mb-2">
                These files exist locally but are not in the mod repository.
                If these are custom textures, copy them to the
                <code className="mx-1 px-1 bg-zinc-700 rounded">user-customs</code>
                folder now before proceeding. The sync will delete these files.
              </p>
              <div className="bg-zinc-900 border border-zinc-700 rounded p-2 max-h-60 overflow-y-auto">
                <ul className="text-xs text-zinc-300 font-mono space-y-0.5">
                  {filesToDelete.map((path) => (
                    <li key={path} className="truncate">{path}</li>
                  ))}
                </ul>
              </div>
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="p-4 border-t border-zinc-700 flex justify-end gap-3">
          <button
            onClick={onCancel}
            className="px-4 py-2 text-sm text-zinc-400 hover:text-zinc-200 transition-colors"
          >
            No, Cancel Sync
          </button>
          <button
            onClick={onConfirm}
            className="px-4 py-2 bg-yellow-600 hover:bg-yellow-500 text-white text-sm font-medium rounded transition-colors"
          >
            Okay, Proceed
          </button>
        </div>
      </div>
    </div>
  );
}

export default SyncWarningDialog;
