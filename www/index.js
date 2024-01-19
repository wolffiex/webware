console.log(Object.entries(window.apiEventSource))
for (const [source, eventStream] of Object.entries(window.apiEventSource)) {
  console.log(source, eventStream)
  let n = 0
  for await (const event of eventStream) {
    if (n++ < 5) console.log("gotone", performance.now(), source, event)
  }
}
