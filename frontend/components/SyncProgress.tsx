import { useEffect, useRef, useState } from "react";

interface SyncProgressPayload {
  stage: string;
  message: string;
  current: number | null;
  total: number | null;
}

interface SyncProgressProps {
  messages: SyncProgressPayload[];
  isComplete: boolean;
  result?: {
    files_downloaded: number;
    files_deleted: number;
    files_skipped: number;
  } | null;
}

const STAGE_LABELS: Record<string, string> = {
  fetching: "Fetching repository info...",
  scanning: "Scanning files...",
  comparing: "Comparing changes...",
  downloading: "Downloading files...",
  deleting: "Removing old files...",
  complete: "Sync complete!",
};

function SyncProgress({ messages, isComplete, result }: SyncProgressProps) {
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const [elapsedTime, setElapsedTime] = useState(0);
  const startTimeRef = useRef<number>(Date.now());

  // Auto-scroll to bottom
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  // Track elapsed time
  useEffect(() => {
    if (isComplete) return;

    const interval = setInterval(() => {
      setElapsedTime(Math.floor((Date.now() - startTimeRef.current) / 1000));
    }, 1000);

    return () => clearInterval(interval);
  }, [isComplete]);

  const formatTime = (seconds: number) => {
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    return mins > 0 ? `${mins}m ${secs}s` : `${secs}s`;
  };

  const lastMessage = messages[messages.length - 1];
  const currentStage = lastMessage?.stage || "fetching";
  const stageLabel = STAGE_LABELS[currentStage] || currentStage;

  // Calculate progress percentage if we have current/total
  const progress = lastMessage?.current && lastMessage?.total
    ? Math.round((lastMessage.current / lastMessage.total) * 100)
    : null;

  return (
    <div className="mt-6 space-y-3">
      {/* Current stage indicator */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          {!isComplete && (
            <svg
              className="animate-spin h-4 w-4 text-blue-400"
              xmlns="http://www.w3.org/2000/svg"
              fill="none"
              viewBox="0 0 24 24"
            >
              <circle
                className="opacity-25"
                cx="12"
                cy="12"
                r="10"
                stroke="currentColor"
                strokeWidth="4"
              />
              <path
                className="opacity-75"
                fill="currentColor"
                d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
              />
            </svg>
          )}
          {isComplete && (
            <svg className="h-4 w-4 text-green-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
            </svg>
          )}
          <span className={`text-sm font-medium ${isComplete ? "text-green-400" : "text-zinc-200"}`}>
            {stageLabel}
          </span>
        </div>
        <span className="text-xs text-zinc-500">
          Elapsed: {formatTime(elapsedTime)}
        </span>
      </div>

      {/* Progress bar (only show when downloading/deleting) */}
      {(currentStage === "downloading" || currentStage === "deleting") && lastMessage?.total && (
        <div className="space-y-1">
          <div className="flex justify-between text-xs text-zinc-400">
            <span>{lastMessage.current} / {lastMessage.total} files</span>
            <span>{progress}%</span>
          </div>
          <div className="w-full h-2 bg-zinc-700 rounded-full overflow-hidden">
            <div
              className="h-full bg-blue-500 transition-all duration-300"
              style={{ width: `${progress}%` }}
            />
          </div>
        </div>
      )}

      {/* Output log */}
      <div className="bg-zinc-950 border border-zinc-700 rounded-lg p-3 max-h-48 overflow-y-auto font-mono text-xs">
        <div className="text-zinc-400 mb-2 pb-2 border-b border-zinc-800">
          Sync Output:
        </div>
        {messages.length === 0 ? (
          <p className="text-zinc-500">Starting sync...</p>
        ) : (
          <div className="space-y-0.5">
            {messages.map((msg, i) => (
              <p
                key={i}
                className={`${
                  msg.stage === "complete"
                    ? "text-green-400"
                    : msg.message.toLowerCase().includes("error")
                    ? "text-red-400"
                    : "text-zinc-300"
                }`}
              >
                {msg.message}
              </p>
            ))}
            <div ref={messagesEndRef} />
          </div>
        )}
      </div>

      {/* Completion summary */}
      {isComplete && result && (
        <div className="bg-green-900/30 border border-green-700 rounded-lg p-3 text-green-300 text-sm">
          <p className="font-medium mb-2">Sync completed successfully!</p>
          <ul className="text-xs space-y-1">
            <li>Files downloaded: {result.files_downloaded}</li>
            <li>Files deleted: {result.files_deleted}</li>
            <li>Files skipped (disabled/up-to-date): {result.files_skipped}</li>
          </ul>
        </div>
      )}
    </div>
  );
}

export default SyncProgress;
