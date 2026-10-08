<script setup>
import { ref, onMounted, onBeforeUnmount, watch, shallowRef } from 'vue'
import { EditorView, keymap, lineNumbers, highlightActiveLine } from '@codemirror/view'
import { EditorState, StateField, StateEffect } from '@codemirror/state'
import { defaultKeymap, history, historyKeymap } from '@codemirror/commands'
import { syntaxHighlighting, defaultHighlightStyle, indentOnInput, bracketMatching, StreamLanguage } from '@codemirror/language'
import { python } from '@codemirror/lang-python'
import { javascript } from '@codemirror/lang-javascript'
import { json } from '@codemirror/lang-json'
import { yaml } from '@codemirror/lang-yaml'
import { go } from '@codemirror/lang-go'
import { markdown } from '@codemirror/lang-markdown'
import { html } from '@codemirror/lang-html'
import { css } from '@codemirror/lang-css'
import { rust } from '@codemirror/lang-rust'
import { cpp } from '@codemirror/lang-cpp'
import { java } from '@codemirror/lang-java'
import { sql } from '@codemirror/lang-sql'
import { php } from '@codemirror/lang-php'
// Legacy stream language modes — for languages without a native CM6 Lezer
// package. Wrapped via StreamLanguage (paths/export names verified against pkg).
import { shell } from '@codemirror/legacy-modes/mode/shell'
import { dockerFile } from '@codemirror/legacy-modes/mode/dockerfile'
import { ruby } from '@codemirror/legacy-modes/mode/ruby'
import { swift } from '@codemirror/legacy-modes/mode/swift'
import { kotlin, scala, dart, objectiveC, csharp } from '@codemirror/legacy-modes/mode/clike'
import { groovy } from '@codemirror/legacy-modes/mode/groovy'
import { r } from '@codemirror/legacy-modes/mode/r'
import { lua } from '@codemirror/legacy-modes/mode/lua'
import { perl } from '@codemirror/legacy-modes/mode/perl'
import { powerShell } from '@codemirror/legacy-modes/mode/powershell'
import { julia } from '@codemirror/legacy-modes/mode/julia'
import { vb } from '@codemirror/legacy-modes/mode/vb'
import { toml } from '@codemirror/legacy-modes/mode/toml'
import { properties } from '@codemirror/legacy-modes/mode/properties'
import { diff } from '@codemirror/legacy-modes/mode/diff'
import { cmake } from '@codemirror/legacy-modes/mode/cmake'
import { stex } from '@codemirror/legacy-modes/mode/stex'
import { oneDark } from '@codemirror/theme-one-dark'
import { Decoration } from '@codemirror/view'
import { useThemeStore } from '../../stores/theme'
import EditorContextMenu from './EditorContextMenu.vue'

/**
 * CodeMirror 6 viewer/editor.
 *
 * Props:
 *   code            — source text
 *   filename        — used to pick a language extension by extension
 *   highlightLines  — array of 1-based line numbers to mark (e.g. leak lines)
 *   editable        — when true, the editor is editable and emits update:modelValue
 *
 * Exposes scrollToLine(n) so the parent can jump to the first leak.
 */
const props = defineProps({
  code: { type: String, default: '' },
  filename: { type: String, default: '' },
  highlightLines: { type: Array, default: () => [] },
  highlightClass: { type: String, default: 'cv-leak-line' }, // CSS class for highlighted lines
  editable: { type: Boolean, default: false },
  /** When false, suppress the built-in editor context menu (useful when the parent provides its own). */
  showContextMenu: { type: Boolean, default: true },
})
const emit = defineEmits(['update:modelValue'])

const host = ref(null)
const view = shallowRef(null)
const theme = useThemeStore()

// ---- right-click menu ----
// The app suppresses the browser's native context menu globally; offer
// copy/select-all/copy-all (and cut/paste when editable) on the editor
// surface so users can still copy code in read-only preview views.
const ctxMenu = ref({ open: false, x: 0, y: 0 })
function onContextmenu(e) {
  e.preventDefault()
  // Don't stopPropagation — let parent dropdowns (IgnoreView, LicenseView)
  // also receive the event. The parent's a-dropdown will fire first.
  if (props.showContextMenu) {
    ctxMenu.value = { open: true, x: e.clientX, y: e.clientY }
  }
}

// ---- language selection by file extension ----
// Legacy stream modes are compiled once and reused — StreamLanguage.define is
// not free, and the editor rebuilds state on every file switch / theme toggle.
const legacyLang = {
  shell: StreamLanguage.define(shell),
  dockerFile: StreamLanguage.define(dockerFile),
  ruby: StreamLanguage.define(ruby),
  swift: StreamLanguage.define(swift),
  kotlin: StreamLanguage.define(kotlin),
  scala: StreamLanguage.define(scala),
  dart: StreamLanguage.define(dart),
  objectiveC: StreamLanguage.define(objectiveC),
  csharp: StreamLanguage.define(csharp),
  groovy: StreamLanguage.define(groovy),
  r: StreamLanguage.define(r),
  lua: StreamLanguage.define(lua),
  perl: StreamLanguage.define(perl),
  powerShell: StreamLanguage.define(powerShell),
  julia: StreamLanguage.define(julia),
  vb: StreamLanguage.define(vb),
  toml: StreamLanguage.define(toml),
  properties: StreamLanguage.define(properties),
  diff: StreamLanguage.define(diff),
  cmake: StreamLanguage.define(cmake),
  stex: StreamLanguage.define(stex),
}

function langExtension(filename) {
  const ext = (filename.split('.').pop() || '').toLowerCase()
  switch (ext) {
    // --- dynamic / scripting ---
    case 'py': return [python()]
    case 'rb': return [legacyLang.ruby]
    case 'pl': case 'pm': return [legacyLang.perl]
    case 'php': case 'phtml': return [php()]
    case 'lua': return [legacyLang.lua]
    case 'r': return [legacyLang.r]
    case 'jl': return [legacyLang.julia]
    case 'sh': case 'bash': case 'zsh': case 'fish': case 'ksh':
      return [legacyLang.shell]
    case 'ps1': case 'psm1': case 'psd1':
      return [legacyLang.powerShell]
    // --- web ---
    case 'js': case 'mjs': case 'cjs': case 'jsx':
      return [javascript()]
    case 'ts': case 'mts': case 'tsx':
      return [javascript({ typescript: true })]
    // vue/sfc: treat as html (handles <template> tags + nested <script>/<style>)
    case 'vue':
      return [html()]
    case 'html': case 'htm': case 'xml': case 'svg': case 'xhtml':
      return [html()]
    case 'css': case 'scss': case 'sass': case 'less':
      return [css()]
    // --- systems / JVM / .NET ---
    case 'go': return [go()]
    case 'rs': return [rust()]
    case 'c': case 'h': return [cpp()]
    case 'cc': case 'cxx': case 'cpp': case 'hpp': case 'hh': case 'ino': case 'ipp':
      return [cpp()]
    case 'm': case 'mm': return [legacyLang.objectiveC]
    case 'java': return [java()]
    case 'kt': case 'kts': return [legacyLang.kotlin]
    case 'scala': case 'sc': return [legacyLang.scala]
    case 'groovy': case 'gradle': return [legacyLang.groovy]
    case 'cs': return [legacyLang.csharp]
    case 'vb': case 'bas': return [legacyLang.vb]
    case 'swift': return [legacyLang.swift]
    case 'dart': return [legacyLang.dart]
    // --- data / query / docs ---
    case 'json': return [json()]
    case 'sql': return [sql()]
    case 'md': case 'markdown': return [markdown()]
    case 'tex': case 'latex': case 'ltx': return [legacyLang.stex]
    // --- config / build ---
    case 'yml': case 'yaml': return [yaml()]
    case 'toml': return [legacyLang.toml]
    case 'ini': case 'cfg': case 'conf': case 'properties': case 'editorconfig':
      return [legacyLang.properties]
    case 'cmake': return [legacyLang.cmake]
    case 'dockerfile': return [legacyLang.dockerFile]
    case 'diff': case 'patch': return [legacyLang.diff]
    default:
      return [] // plain text — no language package
  }
}

// ---- leak-line highlighting via a StateField of line decorations ----
// An effect lets us update the highlighted set without rebuilding the editor.
const setHighlight = StateEffect.define()

const highlightField = StateField.define({
  create() { return Decoration.none },
  update(decos, tr) {
    decos = decos.map(tr.changes)
    for (const e of tr.effects) {
      if (e.is(setHighlight)) {
        decos = e.value
      }
    }
    return decos
  },
  provide: (f) => EditorView.decorations.from(f),
})

// build the Decoration set for a given array of 1-based line numbers
function buildDecos(view, lines) {
  const decos = []
  const doc = view.state.doc
  for (const ln of lines) {
    if (ln < 1 || ln > doc.lines) continue
    const start = doc.line(ln).from
    decos.push(Decoration.line({ class: props.highlightClass }).range(start))
  }
  return Decoration.set(decos, true)
}

function applyHighlight(lines) {
  if (!view.value) return
  const decos = buildDecos(view.value, lines)
  view.value.dispatch({ effects: setHighlight.of(decos) })
}

function themeExtension() {
  return theme.isDark ? [oneDark] : []
}

function buildState() {
  const extensions = [
    EditorView.editable.of(props.editable),
    EditorState.readOnly.of(!props.editable),
    lineNumbers(),
    highlightActiveLine(),
    history(),
    bracketMatching(),
    indentOnInput(),
    syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
    keymap.of([...historyKeymap, ...defaultKeymap.filter(k => k.key !== 'Mod-z' && k.key !== 'Mod-y')]),
    highlightField,
    ...langExtension(props.filename),
    ...themeExtension(),
    // emit content changes when editable
    EditorView.updateListener.of((update) => {
      if (props.editable && update.docChanged) {
        emit('update:modelValue', update.state.doc.toString())
      }
    }),
    EditorView.lineWrapping,
    EditorView.theme({
      '&': { height: '100%', fontSize: '13px' },
      '.cm-scroller': { fontFamily: "'Cascadia Code', 'Fira Code', monospace", lineHeight: '1.6' },
      '.cm-gutters': { borderRight: '1px solid var(--color-border-light)' },
      '.cm-content': { padding: '8px 0' },
    }),
  ]
  return EditorState.create({ doc: props.code || '', extensions })
}

onMounted(() => {
  view.value = new EditorView({
    state: buildState(),
    parent: host.value,
  })
  applyHighlight(props.highlightLines)
})

onBeforeUnmount(() => {
  view.value?.destroy()
})

// react to content / filename changes (e.g. switching files)
watch(() => [props.code, props.filename], () => {
  if (!view.value) return
  view.value.setState(buildState())
  applyHighlight(props.highlightLines)
})

// react to highlight-line changes only (cheap dispatch, no rebuild)
watch(() => props.highlightLines, (lines) => {
  applyHighlight(lines)
}, { deep: true })

// react to theme toggle
watch(() => theme.isDark, () => {
  if (!view.value) return
  view.value.setState(buildState())
  applyHighlight(props.highlightLines)
})

// expose scroll-to-line for the parent
function scrollToLine(n) {
  if (!view.value || n < 1) return
  const doc = view.value.state.doc
  if (n > doc.lines) return
  const line = doc.line(n)
  view.value.dispatch({
    effects: EditorView.scrollIntoView(line.from, { y: 'center' }),
  })
}
defineExpose({ scrollToLine })
</script>

<template>
  <div ref="host" class="cv-host" @contextmenu="onContextmenu"></div>
  <EditorContextMenu
    :open="ctxMenu.open"
    :x="ctxMenu.x"
    :y="ctxMenu.y"
    :view="view"
    :editable="editable"
    @close="ctxMenu.open = false"
  />
</template>

<style scoped>
.cv-host { height: 100%; overflow: hidden; }
.cv-host :deep(.cm-editor) { height: 100%; }
.cv-host :deep(.cm-scroller) { overflow: auto; }
/* leak line highlight — works under both light and dark themes */
.cv-host :deep(.cv-leak-line) {
  background: rgba(255, 85, 85, 0.15);
  border-left: 3px solid var(--color-error);
}
</style>
