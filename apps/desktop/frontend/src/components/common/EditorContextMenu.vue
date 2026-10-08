<script setup>
/**
 * Right-click menu for CodeMirror viewers/editors — restores the copy/cut/
 * paste/select-all/copy-all actions lost when the app suppresses the
 * browser's native context menu globally (main.ts).
 *
 * Works in both editable and read-only modes. In read-only mode (the default
 * for code-preview surfaces like TimeTravel / Backup), cut/paste are disabled
 * and only copy/select-all/copy-all apply.
 *
 * Operations go through the CodeMirror API (not execCommand), so they work
 * reliably inside the editor regardless of focus state.
 *
 * Usage (parent owns the EditorView and a `menu` ref):
 *   <EditorContextMenu :open="menu.open" :x="menu.x" :y="menu.y"
 *     :view="editorView" :editable="true" @close="menu.open = false" />
 */
import { computed } from 'vue'
import ContextMenu from './ContextMenu.vue'
import CtxItem from './CtxItem.vue'
import {
  CopyOutlined, ScissorOutlined, SnippetsOutlined,
  SelectOutlined, BlockOutlined,
} from '@ant-design/icons-vue'

const props = defineProps({
  open: { type: Boolean, default: false },
  x: { type: Number, default: 0 },
  y: { type: Number, default: 0 },
  /** CodeMirror EditorView instance (shallowRef value). */
  view: { type: Object, default: null },
  /** editable mode: enables cut/paste. Defaults false (read-only preview). */
  editable: { type: Boolean, default: false },
})
const emit = defineEmits(['close'])

// selection text + whether anything is selected (drives item disabled state)
const sel = computed(() => {
  const v = props.view
  if (!v) return { text: '', has: false }
  const { from, to } = v.state.selection.main
  const text = from === to ? '' : v.state.sliceDoc(from, to)
  return { text, has: from !== to }
})

function close() { emit('close') }

async function copy() {
  const t = sel.value.text
  if (t) await navigator.clipboard.writeText(t)
  close()
}

async function cut() {
  const v = props.view
  if (!v || !sel.value.has) return
  await navigator.clipboard.writeText(sel.value.text)
  // delete the selection
  v.dispatch(v.state.replaceSelection(''))
  v.focus()
  close()
}

async function paste() {
  const v = props.view
  if (!v) return
  let text = ''
  try { text = await navigator.clipboard.readText() } catch { /* clipboard blocked */ }
  if (text) {
    v.dispatch(v.state.replaceSelection(text))
    v.focus()
  }
  close()
}

function selectAll() {
  const v = props.view
  if (!v) return
  v.dispatch({ selection: { anchor: 0, head: v.state.doc.length } })
  v.focus()
  close()
}

async function copyAll() {
  const v = props.view
  if (!v) return
  await navigator.clipboard.writeText(v.state.doc.toString())
  close()
}
</script>

<template>
  <ContextMenu :open="open" :x="x" :y="y" @update:open="(o) => { if (!o) close() }">
    <CtxItem :disabled="!sel.has" @click="copy">
      <CopyOutlined /> 复制
    </CtxItem>
    <CtxItem v-if="editable" :disabled="!sel.has" @click="cut">
      <ScissorOutlined /> 剪切
    </CtxItem>
    <CtxItem v-if="editable" @click="paste">
      <SnippetsOutlined /> 粘贴
    </CtxItem>
    <div class="ctx-sep"></div>
    <CtxItem @click="selectAll">
      <SelectOutlined /> 全选
    </CtxItem>
    <CtxItem @click="copyAll">
      <BlockOutlined /> 复制全部
    </CtxItem>
  </ContextMenu>
</template>

<style scoped>
/* disabled items: dim + no-cursor; CtxItem renders a <button> */
.ctx-item:disabled,
:deep(button.ctx-item:disabled) {
  opacity: 0.4;
  cursor: not-allowed;
  pointer-events: none;
}
</style>
