import { useState } from "react";

interface SyncDisclaimerDialogProps {
  onAcknowledge: (dontShowAgain: boolean) => void;
}

function SyncDisclaimerDialog({ onAcknowledge }: SyncDisclaimerDialogProps) {
  const [dontShowAgain, setDontShowAgain] = useState(false);

  return (
    <div className="fixed inset-0 bg-black/70 flex items-center justify-center z-50">
      <div className="bg-zinc-800 border border-zinc-600 rounded-lg p-6 max-w-md mx-4 shadow-xl">
        <div className="flex items-start gap-3 mb-4">
          <svg
            className="h-6 w-6 text-yellow-400 flex-shrink-0 mt-0.5"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"
            />
          </svg>
          <h2 className="text-lg font-semibold text-zinc-100">WARNING</h2>
        </div>

        <div className="space-y-3 text-sm text-zinc-300 mb-6">
          <p>
            Sync mode will delete textures that differ from the mod's repo.
          </p>
          <p>
            Put all of your custom textures in the <code className="bg-zinc-700 px-1.5 py-0.5 rounded text-zinc-200">user-customs</code> folder. The sync will never modify anything in that folder.
          </p>
          <p>
            Disable mod-default textures by prepending the filename with a dash (<code className="bg-zinc-700 px-1.5 py-0.5 rounded text-zinc-200">-</code>).
          </p>
        </div>

        <div className="space-y-4">
          <label className="flex items-center gap-2 text-sm text-zinc-400 cursor-pointer">
            <input
              type="checkbox"
              checked={dontShowAgain}
              onChange={(e) => setDontShowAgain(e.target.checked)}
              className="w-4 h-4 rounded bg-zinc-700 border-zinc-600 text-blue-500 focus:ring-blue-500 focus:ring-offset-zinc-800"
            />
            <span>Don't show this again</span>
          </label>

          <button
            onClick={() => onAcknowledge(dontShowAgain)}
            className="w-full py-2.5 bg-blue-600 hover:bg-blue-500 text-white font-medium rounded-lg transition-colors"
          >
            Okay. I understand.
          </button>
        </div>
      </div>
    </div>
  );
}

export default SyncDisclaimerDialog;
