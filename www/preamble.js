console.log(sources)
const queryParams = sources.map(str => `source=${encodeURIComponent(str)}`).join('&');
const eventSource = new EventSource('/api?' + queryParams);
let streamRunning = true
const eventBuffer = [];
eventSource.addEventListener('stream_stop', () => {
  streamRunning = false
  eventSource.close();
});

let resolver = null
eventSource.onmessage = e => {
  eventBuffer.push(e);
  if (resolver) resolver()
};
window.apiEventSource = async function*() {
  console.log('in', eventBuffer);
  while (streamRunning || eventBuffer.length) {
    console.log('inlooo', eventBuffer);
    if (eventBuffer.length > 0) {
      yield eventBuffer.shift();
    } else {
      await new Promise((resolve, _) => {
        resolver = resolve
      });
      resolver = null
    }
  }
}
