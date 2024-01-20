dayjs.extend(dayjs_plugin_relativeTime)
console.log("index.js at", performance.now())
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
  constructor(node, bindings={}) {
    this.node = node
    this.bindings = new Map()
    this.source = null
    for (const [bind, fn] of Object.entries(bindings)) {
      if (bind == "source") {
        this.source = fn(null)
      } else {
        this.bindings.set(bind, fn)
      }
    }
    this.data = null
    this.children = []
  }

  addChild(node, bindings, source) {
    const child = new BindTree(node, bindings, source)
    this.children.push(child)
    return child
  }

  visit(f) {
    f(this)
    for (const child of this.children) {
      child.visit(f)
    }
  }
  bind(data) {
    let hasFailed = false
    const dataProxy = new Proxy(data, {
        get(target, prop, receiver) {
          if (!(prop in target)) {
            hasFailed = true
          }
          return Reflect.get(target, prop, receiver);
        }
    });
    for (const [bind, fn] of this.bindings.entries()) {
      let value
      try {
        hasFailed = false
        const maybeValue = fn(dataProxy)
        if (!hasFailed) value = maybeValue
      } catch(e) {
        console.error("Bound attribute error", e)
      }
      if (value !== undefined) {
        this.applyBinding(bind, value)
      }
    }
    for (const child of this.children) {
      child.inheritBinding(data)
    }
  }
  inheritBinding(data) {
    if (this.source) return
    this.bind(data)
  }
  applyBinding(name, value) {
    switch(name) {
      case "text":
        console.log('set te', value)
        this.node.textContent = value
        break
      default:
        throw new Error("Unrecognized binding", name)
    }
  }
}

function init(bindTree) {
  const sourceBindings = new Map()
  bindTree.visit(treeNode => {
    if (treeNode.source) {
      if (!(sourceBindings.has(treeNode.source))) {
        sourceBindings.set(treeNode.source, [])
      }
      sourceBindings.get(treeNode.source).push(treeNode)
    }
  })
  for (const [source, treeNodes] of sourceBindings) {
    const eventStream = window.apiEventSource[source]
    processEventStream(eventStream, treeNodes)
  }
  console.log('bT', bindTree)
}

async function processEventStream(eventStream, treeNodes) {
  console.log(eventStream, treeNodes)
  for await (const data of eventStream) {
    for (const treeNode of treeNodes) {
      treeNode.bind(data)
    }
  }
}

function findParent(bindSet, node) {
  let p = node
  while (p) {
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
    {
      acceptNode: node => "bound" in node.dataset
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
  } while (node)
  init(bindTree)
}
