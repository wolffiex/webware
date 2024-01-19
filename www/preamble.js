console.log(sources)
const queryParams = sources.map(str => `source=${encodeURIComponent(str)}`).join('&');
const eventSource = new EventSource('/api?' + queryParams);
eventSource.addEventListener('stream_stop', e => {
  eventSource.close();
});

class AsyncStream {
  constructor() {
    this.resolver = null
    this.streamRunning = true
    this.buffer = []
  }

  push(v) {
    this.buffer.push(v)
    // console.log(this, this.buffer, this.resolver)
    if (this.resolver) this.resolver()
  }

  close() {
    this.streamRunning = false
    if (this.resolver) this.resolver()
  }

  async *[Symbol.asyncIterator]() {
    while (this.streamRunning || this.buffer.length > 0) {
      if (this.buffer.length > 0) {
        yield this.buffer.shift();
      } else {
        await new Promise((resolve, _) => {
          this.resolver = resolve
        });
        this.resolver = null
      }
    }
  }
}

window.apiEventSource = Object.fromEntries(sources.map(source => {
  const stream = new AsyncStream()
  eventSource.addEventListener(source, e => {
    stream.push(e)
  })
  eventSource.addEventListener('stream_stop', () => {
    console.log('wanna clo', source, stream)
    stream.close()
  });
  return [source, stream]
}))

console.log(window.apiEventSource)
