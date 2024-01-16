function connect(...eventNames) {
  const eventSource = new EventSource('/api');
  let streamRunning = true
  const eventBuffer = [];
  eventSource.addEventListener('stream_stop', () => {
    streamRunning = false
    eventSource.close();
  });

  let resolver = null
  eventSource.onmessage = e => {
    eventBuffer.push(e);
    const currentResolver = resolver
    resolver = null
    if (currentResolver) currentResolver()
  };
  return async function*() {
    while (streamRunning || eventBuffer.length) {
      if (eventBuffer.length > 0) {
        yield eventBuffer.shift();
      } else {
        await new Promise((resolve, _) => {
          resolver = resolve
        });
      }
    }
  }
}
