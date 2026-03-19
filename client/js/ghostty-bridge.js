import { init, Terminal } from '/ghostty-web.js';

let initialized = false;

export class GhosttyTerminal {
    constructor() {
        this.term = null;
        this.container = null;
        this.cellWidth = 0;
        this.cellHeight = 0;
        this._onDataCb = null;
        this._onResizeCb = null;
    }

    async init() {
        if (!initialized) {
            await init();
            initialized = true;
        }
        this.term = new Terminal({ fontSize: 14 });
    }

    open(element) {
        this.container = element;
        this.term.open(element);
    }

    // Must be called after open + a frame delay to measure cell size
    measureCells() {
        const canvas = this.container?.querySelector('canvas');
        if (canvas && canvas.clientWidth > 0) {
            this.cellWidth = canvas.clientWidth / 80;
            this.cellHeight = canvas.clientHeight / 24;
        }
    }

    writeBytes(data) {
        this.term.write(data);
    }

    writeString(data) {
        this.term.write(data);
    }

    resize(cols, rows) {
        this.term.resize(cols, rows);
    }

    dispose() {
        if (this.term) {
            this.term.dispose();
            this.term = null;
        }
    }

    onData(callback) {
        this._onDataCb = callback;
        this.term.onData((data) => callback(data));
    }

    onResize(callback) {
        this._onResizeCb = callback;
        this.term.onResize(({ cols, rows }) => callback(cols, rows));
    }

    fitToContainer() {
        if (!this.cellWidth) this.measureCells();
        if (!this.container || !this.cellWidth) return null;
        const rect = this.container.getBoundingClientRect();
        const cols = Math.max(2, Math.floor(rect.width / this.cellWidth));
        const rows = Math.max(1, Math.floor(rect.height / this.cellHeight));
        this.term.resize(cols, rows);
        return { cols, rows };
    }
}
