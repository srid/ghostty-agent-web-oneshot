import React, { useState } from 'react';

export default function NewSessionDialog({ loading, onCreate, onCancel }) {
  const [command, setCommand] = useState('');
  const [workingDir, setWorkingDir] = useState('');
  const [variant, setVariant] = useState('shell');

  const handleSubmit = (e) => {
    e.preventDefault();
    const data = { variant };
    if (command.trim()) data.command = command.trim();
    if (workingDir.trim()) data.cwd = workingDir.trim();
    onCreate(data);
  };

  return (
    <div className="dialog-overlay" onClick={onCancel}>
      <div className="dialog" onClick={(e) => e.stopPropagation()}>
        <h2>New Session</h2>
        <form onSubmit={handleSubmit}>
          <label>
            Variant
            <select value={variant} onChange={(e) => setVariant(e.target.value)}>
              <option value="shell">shell</option>
              <option value="opencode">opencode</option>
              <option value="claude-code">claude-code</option>
            </select>
          </label>
          <label>
            Command
            <input
              type="text"
              value={command}
              onChange={(e) => setCommand(e.target.value)}
              placeholder="Leave empty for default shell"
            />
          </label>
          <label>
            Working Directory
            <input
              type="text"
              value={workingDir}
              onChange={(e) => setWorkingDir(e.target.value)}
              placeholder="Default: home directory"
            />
          </label>
          <div className="dialog-actions">
            <button type="button" className="cancel-btn" onClick={onCancel}>
              Cancel
            </button>
            <button type="submit" className="create-btn" disabled={loading}>
              {loading ? 'Creating...' : 'Create'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
