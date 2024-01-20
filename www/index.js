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
class BindTree {
  constructor(node, binding) {
    this.node = node
    this.binding = binding || {}
    this.children = []
  }
  addChild(node, binding) {
    const child = new BindTree(node, binding)
    this.children.push(child)
    return child
  }
}

function init(bindTree) {
  console.log('bT', bindTree)
}

function findParent(bindSet, node) {
  let p = node
  while(p) {
    if (bindSet.has(p)) {
      return bindSet.get(p)
    }
    p = p.parentNode
  } 
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
  const bindMap = new Map()
  const bindTree = new BindTree(document.body)
  let node, i = 0
  do {
    node = walker.nextNode();
    if (node) {
      const boundParent = findParent(bindMap, node) || bindTree
      const binding = boundParent.addChild(node, bindings[i++])
      bindMap.set(node, binding)
    }
  } while(node)
  init(bindTree)
}
