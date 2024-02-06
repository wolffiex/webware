const queryParams = sources
  .map((str) => `source=${encodeURIComponent(str)}`)
  .join("&");
const eventSource = new EventSource("/api?" + queryParams);
eventSource.addEventListener("stream_stop", (e) => {
  eventSource.close();
});

class AsyncStream {
  constructor() {
    this.resolver = null;
    this.streamRunning = true;
    this.buffer = [];
  }

  push(v) {
    this.buffer.push(v);
    this._aContinue()
  }

  close() {
    this.streamRunning = false;
    this._aContinue()
  }

  _aContinue() {
    if (this.resolver) this.resolver();
    this.resolver = null;
  }

  async *[Symbol.asyncIterator]() {
    while (this.streamRunning || this.buffer.length > 0) {
      if (this.buffer.length > 0) {
        yield this.buffer.shift();
      } else {
        await new Promise((resolve, _) => {
          this.resolver = resolve;
        });
      }
    }
  }
}

window.apiEventSource = Object.fromEntries(
  sources.map((source) => {
    const stream = new AsyncStream();
    eventSource.addEventListener(source, (e) => {
      stream.push(JSON.parse(e.data));
    });
    eventSource.addEventListener("stream_stop", () => {
      stream.close();
    });
    return [source, stream];
  }),
);

console.log(window.apiEventSource);
console.log("preamble at", performance.now());
