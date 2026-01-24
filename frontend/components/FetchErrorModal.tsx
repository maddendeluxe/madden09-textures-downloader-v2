interface FetchErrorModalProps {
  error: string;
  onRetry: () => void;
}

function FetchErrorModal({ error, onRetry }: FetchErrorModalProps) {
  return (
    <div className="fixed inset-0 bg-black/80 flex items-center justify-center z-50">
      <div className="bg-zinc-800 border border-yellow-600 rounded-lg p-6 max-w-md mx-4 shadow-xl">
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
              d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
            />
          </svg>
          <h2 className="text-lg font-semibold text-yellow-400">Connection Error</h2>
        </div>

        <div className="space-y-3 text-sm text-zinc-300 mb-6">
          <p>
            Unable to fetch required configuration data from the texture repository.
          </p>
          <div className="bg-zinc-900/50 rounded-lg p-3">
            <p className="text-zinc-400 text-xs font-mono break-all">{error}</p>
          </div>
          <p>
            Please check your internet connection and try again.
          </p>
        </div>

        <button
          onClick={onRetry}
          className="w-full py-2.5 bg-blue-600 hover:bg-blue-500 text-white font-medium rounded-lg transition-colors"
        >
          Retry
        </button>
      </div>
    </div>
  );
}

export default FetchErrorModal;
