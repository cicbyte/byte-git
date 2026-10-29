<script setup>
/**
 * Recursive file-tree node renderer for the backup browser.
 * Renders a list of nodes at a given depth; for expanded directories it
 * recurses into their children.
 *
 * Styling follows the VSCode explorer: dirs first, file-type icon colors,
 * hover/active states, chevron rotation.
 */
import { computed } from 'vue'
import {
  FolderFilled, FileOutlined, FileTextOutlined, FileMarkdownOutlined,
  CodeOutlined, FileZipOutlined, DownOutlined, RightOutlined,
  CheckOutlined, StopOutlined,
} from '@ant-design/icons-vue'

const props = defineProps({
  nodes: { type: Array, default: () => [] },
  depth: { type: Number, default: 0 },
  tree: { type: Object, default: () => ({}) },
  expanded: { type: Set, default: () => new Set() },
  selected: { default: '' }, // string (single) or Set (multi-select)
  // optional badge map: { [path]: 'tracked'|'ignored'|'untracked' }
  badges: { type: Object, default: null },
  // checkbox support: when true, each file node shows a checkbox bound to
  // checkedPaths; checking emits 'check'/'uncheck' without triggering open.
  selectable: { type: Boolean, default: false },
  checkedPaths: { type: Set, default: null },
})
const emit = defineEmits(['toggle', 'open', 'contextmenu', 'check', 'uncheck'])

// normalize selected to a "is this path selected?" check
function isSelected(path) {
  if (!props.selected) return false
  if (typeof props.selected === 'string') return props.selected === path
  if (props.selected instanceof Set) return props.selected.has(path)
  return false
}

function isChecked(path) {
  return props.checkedPaths instanceof Set && props.checkedPaths.has(path)
}

// click: always select (highlight) the node; for dirs also toggle expand
function onNodeClick(node) {
  emit('open', node)        // select (works for both files and dirs)
  if (node.isDir) emit('toggle', node)
}

// checkbox click: stop propagation so it doesn't select/open the node
function onCheckClick(e, node) {
  e.stopPropagation()
  if (isChecked(node.path)) {
    emit('uncheck', node)
  } else {
    emit('check', node)
  }
}

// pick a file-type icon + color by extension (VSCode-ish)
function fileIcon(name) {
  if (!name) return { icon: FileOutlined, color: 'var(--color-text-tertiary)' }
  const ext = (name.split('.').pop() || '').toLowerCase()
  const map = {
    go: { icon: CodeOutlined, color: '#00ADD8' },
    js: { icon: CodeOutlined, color: '#F7DF1E' },
    ts: { icon: CodeOutlined, color: '#3178C6' },
    vue: { icon: CodeOutlined, color: '#42B883' },
    py: { icon: CodeOutlined, color: '#3776AB' },
    rs: { icon: CodeOutlined, color: '#DEA584' },
    md: { icon: FileMarkdownOutlined, color: '#6C6C6C' },
    json: { icon: CodeOutlined, color: '#CBC24A' },
    yml: { icon: FileTextOutlined, color: '#CB171E' },
    yaml: { icon: FileTextOutlined, color: '#CB171E' },
    toml: { icon: FileTextOutlined, color: '#9C4221' },
    html: { icon: CodeOutlined, color: '#E34F26' },
    css: { icon: CodeOutlined, color: '#1572B6' },
    zip: { icon: FileZipOutlined, color: '#7B68EE' },
    gz: { icon: FileZipOutlined, color: '#7B68EE' },
    png: { icon: FileOutlined, color: '#A8A8A8' },
    jpg: { icon: FileOutlined, color: '#A8A8A8' },
  }
  return map[ext] || { icon: FileOutlined, color: 'var(--color-text-tertiary)' }
}
</script>

<template>
  <template v-for="node in nodes" :key="node.path">
    <div
      class="ftn-node"
      :class="{ selected: isSelected(node.path), 'is-dir': node.isDir }"
      :style="{ paddingLeft: 6 + depth * 14 + 'px' }"
      @click="onNodeClick(node)"
      @contextmenu="emit('contextmenu', node, $event)"
    >
      <!-- chevron (only for dirs) -->
      <span class="ftn-chevron" v-if="node.isDir">
        <DownOutlined v-if="expanded.has(node.path)" />
        <RightOutlined v-else />
      </span>
      <span class="ftn-chevron-spacer" v-else></span>

      <!-- optional checkbox (files only, when selectable) -->
      <input
        v-if="selectable && !node.isDir"
        type="checkbox"
        class="ftn-check"
        :checked="isChecked(node.path)"
        @click="onCheckClick($event, node)"
      />
      <span v-if="selectable && node.isDir" class="ftn-check-spacer"></span>

      <!-- icon -->
      <component
        :is="node.isDir ? FolderFilled : fileIcon(node.name).icon"
        class="ftn-icon"
        :style="!node.isDir ? { color: fileIcon(node.name).color } : {}"
      />

      <span class="ftn-name" :title="node.name">{{ node.name }}</span>
      <!-- optional status badge (from badges prop) -->
      <span v-if="badges && badges[node.path]" class="ftn-badge" :class="'bdg-' + badges[node.path]">
        <CheckOutlined v-if="badges[node.path] === 'tracked'" />
        <StopOutlined v-else-if="badges[node.path] === 'ignored'" />
        <span v-else class="ftn-dot" />
      </span>
      <!-- optional caller-provided extra content (e.g. A/M/D status, deltas) -->
      <slot name="extra" :node="node"></slot>
    </div>

    <!-- recurse into expanded dirs -->
    <FileTreeNode
      v-if="node.isDir && expanded.has(node.path)"
      :nodes="tree[node.path] || []"
      :depth="depth + 1"
      :tree="tree"
      :expanded="expanded"
      :selected="selected"
      :badges="badges"
      :selectable="selectable"
      :checked-paths="checkedPaths"
      @toggle="(n) => emit('toggle', n)"
      @open="(n) => emit('open', n)"
      @contextmenu="(n, e) => emit('contextmenu', n, e)"
      @check="(n) => emit('check', n)"
      @uncheck="(n) => emit('uncheck', n)"
    >
      <template v-if="$slots.extra" #extra="{ node: child }"><slot name="extra" :node="child" /></template>
    </FileTreeNode>
  </template>
</template>

<style scoped>
.ftn-node {
  display: flex;
  align-items: center;
  gap: 5px;
  padding: 3px 8px;
  cursor: pointer;
  font-size: 13px;
  line-height: 1.5;
  color: var(--color-text);
  white-space: nowrap;
  user-select: none;
  border-radius: var(--radius-xs);
  transition: background var(--transition-fast);
}
.ftn-node:hover { background: var(--color-hover); }
.ftn-node.selected { background: var(--color-primary-bg); }
.ftn-node.selected .ftn-name { color: var(--color-primary); font-weight: 500; }

.ftn-chevron {
  width: 16px;
  height: 16px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  color: var(--color-text-tertiary);
  font-size: 9px;
  transition: transform var(--transition-fast);
}
.ftn-chevron-spacer { width: 16px; flex-shrink: 0; }

.ftn-check { flex-shrink: 0; width: 14px; height: 14px; cursor: pointer; }
.ftn-check-spacer { flex-shrink: 0; width: 14px; }

.ftn-icon { font-size: 15px; flex-shrink: 0; color: var(--color-warning); }
.ftn-name { overflow: hidden; text-overflow: ellipsis; flex: 1; }
.ftn-badge { font-size: 11px; flex-shrink: 0; margin-left: 4px; display: inline-flex; align-items: center; }
.ftn-badge.bdg-tracked { color: var(--color-success); }
.ftn-badge.bdg-ignored { color: var(--color-error); }
.ftn-badge.bdg-untracked { color: var(--color-warning); }
.ftn-dot { width: 8px; height: 8px; border: 1.5px solid currentColor; border-radius: 50%; }
</style>
