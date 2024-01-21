console.log("index.js at", performance.now());
class DataNotFoundError extends Error {
  constructor(message) {
    super(message);
    this.name = 'DataNotFoundError';
  }
}
class BindTree {
  constructor(node, bindings = {}) {
    this.node = node;
    this.bindings = bindings;
    this.data = null;
    this.children = [];
    this.source = null;
    if (bindings.module) {
      const source = bindings.source ? new AsyncStream() : null;
      bindings.module({ node, source });
      this.source = source;
    }
  }

  addChild(node, bindings, source) {
    const child = new BindTree(node, bindings, source);
    this.children.push(child);
    return child;
  }

  visit(f) {
    f(this);
    for (const child of this.children) {
      child.visit(f);
    }
  }
  bind(data) {
    const dataProxy = new Proxy(data, {
      get(target, prop, receiver) {
        if (!(prop in target)) {
          throw new DataNotFoundError(prop);
        }
        return Reflect.get(target, prop, receiver);
      },
    });
    for (const [bind, fn] of Object.entries(this.bindings.dynamic || {})) {
      let value;
      try {
        value = fn(dataProxy);
      } catch (e) {
        if (e.name === 'DataNotFoundError') {
          // Intentionally ignored
        } else {
          console.error("Bound attribute error", e);
        }
      }
      if (value !== undefined) {
        this.applyBinding(bind, value);
      }
    }
    for (const child of this.children) {
      child.inheritBinding(data);
    }
  }
  inheritBinding(data) {
    if (!this.bindings.source) this.bind(data);
  }
  applyBinding(name, value) {
    switch (name) {
      case "text":
        this.node.textContent = value;
        break;
      default:
        throw new Error("Unrecognized binding", name);
    }
  }
}

function init(bindTree) {
  const sourceBindings = new Map();
  bindTree.visit((treeNode) => {
    if (treeNode.bindings.source) {
      if (!sourceBindings.has(treeNode.bindings.source)) {
        sourceBindings.set(treeNode.bindings.source, []);
      }
      sourceBindings.get(treeNode.bindings.source).push(treeNode);
    }
  });
  for (const [source, treeNodes] of sourceBindings) {
    const eventStream = window.apiEventSource[source];
    processEventStream(eventStream, treeNodes);
  }
  console.log("bT", bindTree);
}

async function processEventStream(eventStream, treeNodes) {
  for await (const data of eventStream) {
    for (const treeNode of treeNodes) {
      treeNode.source?.push(data);
      treeNode.bind(data);
    }
  }
  for (const treeNode of treeNodes) {
    treeNode.source?.close();
  }
}

function findParent(bindSet, node) {
  let p = node;
  while (p) {
    if (bindSet.has(p)) {
      return bindSet.get(p);
    }
    p = p.parentNode;
  }
}

export default function (...bindings) {
  const walker = document.createTreeWalker(
    document.body, // root
    NodeFilter.SHOW_ELEMENT, // filter
    {
      acceptNode: (node) =>
        "bound" in node.dataset
          ? NodeFilter.FILTER_ACCEPT
          : NodeFilter.FILTER_SKIP,
    }, // node filter function
    false,
  );
  const bindMap = new Map();
  const bindTree = new BindTree(document.body);
  let node,
    i = 0;
  do {
    node = walker.nextNode();
    if (node) {
      const boundParent = findParent(bindMap, node) || bindTree;
      const binding = boundParent.addChild(node, bindings[i++]);
      bindMap.set(node, binding);
    }
  } while (node);
  init(bindTree);
}
