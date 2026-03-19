import React, { useRef, useEffect } from 'react';

export default function TerminalView({ sessionId }) {
  const containerRef = useRef(null);

  useEffect(() => {
    if (!sessionId || !containerRef.current) return;

    let disposed = false;
    let terminal = null;
    let ws = null;
    let resizeObserver = null;
    let cellWidth = 0;
    let cellHeight = 0;

    const fit = () => {
      if (disposed || !terminal || !cellWidth) return null;
      const rect = containerRef.current.getBoundingClientRect();
      const cols = Math.max(2, Math.floor(rect.width / cellWidth));
      const rows = Math.max(1, Math.floor(rect.height / cellHeight));
      try { terminal.resize(cols, rows); } catch {}
      return { cols, rows };
    };

    const setup = async () => {
      const { init, Terminal } = await import('ghostty-web');
      await init();
      if (disposed) return;

      const container = containerRef.current;

      // Create at default 80x24
      terminal = new Terminal({ fontSize: 14 });
      terminal.open(container);

      // Derive cell size from the rendered canvas (80x24 default)
      // Wait a frame for canvas to be in the DOM
      await new Promise((r) => requestAnimationFrame(r));
      const canvas = container.querySelector('canvas');
      if (canvas) {
        cellWidth = canvas.clientWidth / 80;
        cellHeight = canvas.clientHeight / 24;
      }

      // Fit to container
      const size = cellWidth ? fit() : null;

      // Connect WebSocket
      const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
      ws = new WebSocket(`${protocol}//${window.location.host}/ws/${sessionId}`);
      ws.binaryType = 'arraybuffer';

      ws.onopen = () => {
        if (size) {
          ws.send(JSON.stringify({ type: 'resize', cols: size.cols, rows: size.rows }));
        }
      };

      ws.onmessage = (e) => {
        if (disposed) return;
        if (e.data instanceof ArrayBuffer) {
          terminal.write(new Uint8Array(e.data));
        } else {
          try {
            const msg = JSON.parse(e.data);
            if (msg.type === 'exit') {
              terminal.write('\r\n\x1b[90m[session exited]\x1b[0m\r\n');
              return;
            }
          } catch {}
          terminal.write(e.data);
        }
      };

      ws.onclose = () => {};
      ws.onerror = () => {};

      terminal.onData((data) => {
        if (ws.readyState === WebSocket.OPEN) {
          ws.send(data);
        }
      });

      terminal.onResize(({ cols, rows }) => {
        if (ws.readyState === WebSocket.OPEN) {
          ws.send(JSON.stringify({ type: 'resize', cols, rows }));
        }
      });

      resizeObserver = new ResizeObserver(() => {
        const s = fit();
        if (s && ws && ws.readyState === WebSocket.OPEN) {
          ws.send(JSON.stringify({ type: 'resize', cols: s.cols, rows: s.rows }));
        }
      });
      resizeObserver.observe(container);
    };

    setup().catch((err) => {
      console.error('Failed to setup terminal:', err);
    });

    return () => {
      disposed = true;
      if (resizeObserver) resizeObserver.disconnect();
      if (ws && ws.readyState <= WebSocket.OPEN) ws.close();
      if (terminal) { try { terminal.dispose(); } catch {} }
    };
  }, [sessionId]);

  return <div ref={containerRef} className="terminal-container" />;
}
