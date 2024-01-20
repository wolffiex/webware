console.log("index.js at", performance.now())
console.log(Object.entries(window.apiEventSource))
/*
for (const [source, eventStream] of Object.entries(window.apiEventSource)) {
  console.log(source, eventStream)
  let n = 0
  for await (const event of eventStream) {
    if (n++ < 5) console.log("gotone", performance.now(), source, event)
  }
}
*/

function bind(node, binding) {
  console.log("BB", node, binding)
}

export default function(...bindings) {
  const walker = document.createTreeWalker(
    document.body, // root
    NodeFilter.SHOW_ELEMENT, // filter
    { acceptNode: node => "bound" in node.dataset
      ? NodeFilter.FILTER_ACCEPT
      : NodeFilter.FILTER_SKIP
    }, // node filter function
    false
  )
  let node, i = 0
  do {
    node = walker.nextNode();
    if (node) bind(node, bindings[i])
  } while(node)
}
