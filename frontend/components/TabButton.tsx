interface TabButtonProps {
  label: string;
  isActive: boolean;
  onClick: () => void;
  disabled?: boolean;
}

function TabButton({ label, isActive, onClick, disabled }: TabButtonProps) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      className={`
        px-6 py-2 text-sm font-medium rounded-t-lg transition-colors
        ${isActive
          ? "bg-zinc-800 text-zinc-100 border-t border-l border-r border-zinc-600"
          : "bg-zinc-900 text-zinc-400 hover:text-zinc-300 hover:bg-zinc-800/50"
        }
        ${disabled ? "opacity-50 cursor-not-allowed" : "cursor-pointer"}
      `}
    >
      {label}
    </button>
  );
}

export default TabButton;
