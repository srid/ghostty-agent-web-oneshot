import React, { useState, useEffect, useCallback } from 'react';
import TerminalView from './TerminalView';
import NewSessionDialog from './NewSessionDialog';

export default function App() {
  const [sessions, setSessions] = useState([]);
  const [selectedSessionId, setSelectedSessionId] = useState(null);
  const [showNewDialog, setShowNewDialog] = useState(false);
  const [loading, setLoading] = useState(false);

  const fetchSessions = useCallback(async () => {
    try {
      const res = await fetch('/api/sessions');
      if (res.ok) {
        const data = await res.json();
        setSessions(data);
      }
    } catch (err) {
      console.error('Failed to fetch sessions:', err);
    }
  }, []);

  useEffect(() => {
    fetchSessions();
    const interval = setInterval(fetchSessions, 3000);
    return () => clearInterval(interval);
  }, [fetchSessions]);

  const handleCreateSession = async (formData) => {
    setLoading(true);
    try {
      const res = await fetch('/api/sessions', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(formData),
      });
      if (res.ok) {
        const session = await res.json();
        setShowNewDialog(false);
        setSelectedSessionId(session.id);
        await fetchSessions();
      }
    } catch (err) {
      console.error('Failed to create session:', err);
    } finally {
      setLoading(false);
    }
  };

  const handleDeleteSession = async (e, id) => {
    e.stopPropagation();
    try {
      await fetch(`/api/sessions/${id}`, { method: 'DELETE' });
      if (selectedSessionId === id) {
        setSelectedSessionId(null);
      }
      await fetchSessions();
    } catch (err) {
      console.error('Failed to delete session:', err);
    }
  };

  const basename = (path) => {
    if (!path) return '';
    return path.split('/').filter(Boolean).pop() || path;
  };

  return (
    <div className="app">
      <aside className="sidebar">
        <div className="sidebar-header">
          <h1>ghostty-agent-web</h1>
        </div>
        <button
          className="new-session-btn"
          onClick={() => setShowNewDialog(true)}
        >
          + New Session
        </button>
        <div className="session-list">
          {sessions.map((s) => (
            <div
              key={s.id}
              className={`session-item ${selectedSessionId === s.id ? 'selected' : ''}`}
              onClick={() => setSelectedSessionId(s.id)}
            >
              <div className="session-info">
                <span className="session-id">{s.id.slice(0, 8)}</span>
                <span
                  className={`status-badge ${s.status === 'running' ? 'running' : 'exited'}`}
                />
              </div>
              <div className="session-meta">
                <span className="session-command">{s.command || 'shell'}</span>
                {s.cwd && (
                  <span className="session-dir">{basename(s.cwd)}</span>
                )}
              </div>
              <button
                className="delete-btn"
                onClick={(e) => handleDeleteSession(e, s.id)}
                title="Delete session"
              >
                &times;
              </button>
            </div>
          ))}
        </div>
      </aside>

      <main className="terminal-area">
        {selectedSessionId ? (
          <TerminalView key={selectedSessionId} sessionId={selectedSessionId} />
        ) : (
          <div className="placeholder">Select or create a session</div>
        )}
      </main>

      {showNewDialog && (
        <NewSessionDialog
          loading={loading}
          onCreate={handleCreateSession}
          onCancel={() => setShowNewDialog(false)}
        />
      )}
    </div>
  );
}
