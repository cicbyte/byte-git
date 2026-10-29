<script setup>
/**
 * File tree picker — renders a repo's files as an expandable tree with
 * click-to-select (multi-select). Self-contained: manages tree building,
 * expand state, search auto-expand, and selection. Only renders expanded
 * subtrees for performance with large file counts.
 *
 * Props:
 *   files    — flat array of file paths (e.g. from `git ls-files`)
 *   exclude  — Set of top-level names to hide (e.g. .git)
 *   search   — search query (auto-expands matching files' ancestors)
 *
 * Emits:
 *   update:selected — array of selected file paths
 *
 * Exposes:
 *   clearSelection()
 */
import { ref, computed, watch } from 'vue'
import { SearchOutlined } from '@ant-design/icons-vue'
import FileTreeNode from './FileTreeNode.vue'

const props = defineProps({
  files: { type: Array, default: () => [] },
  exclude: { type: Set, default: () => new Set() },
  search: { type: String, default: '' },
  // status badges: { [path]: 'tracked'|'ignored'|'untracked' }
  badges: { type: Object, default: null },
  // paths known to be directories (e.g. collapsed ignored dirs like
  // node_modules that have no children in the file list). Without this, the
  // tree builder infers dir/file purely from path nesting and would render a
  // childless dir as a file.
  dirSet: { type: Set, default: () => new Set() },
})
const emit = defineEmits(['check-file'])

// ---- build tree from flat paths ----
const treeMap = computed(() => {
  const root = []
  const dirMap = { '__root__': root }

  // nodeMap: path → node object (for ensureDir to return)
  // dirMap: path → children array (for FileTreeNode's tree[path] lookup)
  const nodeMap = {}

  const ensureDir = (fullPath) => {
    if (nodeMap[fullPath]) return nodeMap[fullPath]
    const parts = fullPath.split('/').filter(s => s !== '')
    if (!parts.length) return null
    const node = { name: parts[parts.length - 1], path: parts.join('/'), isDir: true, children: [] }
    nodeMap[fullPath] = node
    dirMap[node.path] = node.children  // FileTreeNode looks up children by path
    if (parts.length === 1) {
      root.push(node)
    } else {
      const parent = ensureDir(parts.slice(0, -1).join('/'))
      if (parent) parent.children.push(node)
    }
    return node
  }

  for (const p of props.files) {
    if (!p || !p.trim()) continue
    const parts = p.split('/').filter(s => s !== '')
    if (!parts.length) continue
    if (props.exclude.has(parts[0])) continue
    // a collapsed dir (e.g. node_modules) arrives as a leaf path but must be
    // rendered as a directory — check dirSet before treating it as a file.
    const isKnownDir = props.dirSet && props.dirSet.has(p)
    if (isKnownDir) {
      // children may have created this dir already — reuse to avoid duplicates
      const existing = nodeMap[p]
      if (existing) {
        existing.isDir = true
        continue
      }
      const node = { name: parts[parts.length - 1], path: p, isDir: true, children: [] }
      nodeMap[p] = node
      dirMap[p] = node.children
      if (parts.length === 1) {
        root.push(node)
      } else {
        const parent = ensureDir(parts.slice(0, -1).join('/'))
        if (parent) parent.children.push(node)
      }
    } else if (parts.length === 1) {
      root.push({ name: parts[0], path: parts[0], isDir: false })
    } else {
      const parent = ensureDir(parts.slice(0, -1).join('/'))
      if (parent) parent.children.push({ name: parts[parts.length - 1], path: parts.join('/'), isDir: false })
    }
  }

  const sortNodes = (nodes) => {
    nodes.sort((a, b) => {
      if (a.isDir !== b.isDir) return a.isDir ? -1 : 1
      return a.name.toLowerCase().localeCompare(b.name.toLowerCase())
    })
    nodes.forEach(n => n.children && sortNodes(n.children))
  }
  sortNodes(root)
  return dirMap
})

// ---- expand state ----
const expanded = ref(new Set())

// when searching, auto-expand all ancestors of matching files
watch(() => props.search, (q) => {
  if (!q || !q.trim()) return
  const ql = q.toLowerCase()
  const toExpand = new Set()
  for (const f of props.files) {
    if (f.toLowerCase().includes(ql)) {
      const parts = f.split('/')
      for (let i = 1; i < parts.length; i++) {
        toExpand.add(parts.slice(0, i).join('/'))
      }
    }
  }
  expanded.value = new Set([...expanded.value, ...toExpand])
})

function onToggle(node) {
  if (expanded.value.has(node.path)) expanded.value.delete(node.path)
  else expanded.value.add(node.path)
  expanded.value = new Set(expanded.value)
}

// ---- selection (single-select highlight) ----
const selectedPath = ref('')

function onOpen(node) {
  // single click = select (highlight) the file; right-click = check
  selectedPath.value = node.path
}

function clearSelection() {
  selectedPath.value = ''
}

// ---- right-click context menu ----
const ctxOpen = ref(false)
const ctxFile = ref('')

function onContextmenu(node, evt) {
  evt.preventDefault()
  ctxFile.value = node.path
  ctxOpen.value = true
}

function ctxCheck() {
  ctxOpen.value = false
  emit('check-file', ctxFile.value)
}

defineExpose({ clearSelection })
</script>

<template>
  <div class="ftp">
    <!-- context menu is manually controlled via ctxOpen (trigger=[] keeps hover
         from popping it; contextmenu would double-open when onContextmenu also
         sets ctxOpen=true) -->
    <a-dropdown :open="ctxOpen" :trigger="[]" @open-change="(v) => ctxOpen = v">
      <div class="ftp-tree-host">
        <FileTreeNode
          :nodes="treeMap['__root__'] || []"
          :depth="0"
          :tree="treeMap"
          :expanded="expanded"
          :selected="selectedPath"
          :badges="badges"
          @toggle="onToggle"
          @open="onOpen"
          @contextmenu="onContextmenu"
        />
        <div v-if="!files.length" class="ftp-empty">无文件</div>
      </div>
      <template #overlay>
        <a-menu>
          <a-menu-item @click="ctxCheck">
            <SearchOutlined /> 检查是否被忽略
          </a-menu-item>
        </a-menu>
      </template>
    </a-dropdown>
  </div>
</template>

<style scoped>
.ftp { height: 100%; overflow-y: auto; padding: 4px; }
.ftp-tree-host { min-height: 100%; }
.ftp-empty { padding: 24px; text-align: center; font-size: 12px; color: var(--color-text-tertiary); }
/* selected files (multi-select) get primary bg */
.ftp :deep(.ftn-node.selected) { background: var(--color-primary-bg); }
.ftp :deep(.ftn-node.selected .ftn-name) { color: var(--color-primary); font-weight: 500; }
</style>
