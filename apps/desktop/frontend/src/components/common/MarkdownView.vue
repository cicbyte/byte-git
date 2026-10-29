<script setup>
import { computed, ref, onMounted, watch, nextTick } from 'vue'
import { marked } from 'marked'
import DOMPurify from 'dompurify'

/**
 * Renders a markdown string as styled HTML.
 *
 * Used in three places:
 *   - HomeView: README preview card
 *   - InfoView: README tab
 *   - BackupView: browsing .md files inside a full-clone backup
 *
 * Styling uses project CSS variables so it adapts to light/dark themes.
 * All output is sandboxed via scoped styles; external links open in a new tab.
 */
const props = defineProps({
  source: { type: String, default: '' },
  filename: { type: String, default: '' }, // .md 路径,用于解析相对图片的基准目录
  resolveImage: { type: Function, default: null }, // (rel)=>Promise<dataUrl>;不传则用 RepoReadImage(当前仓库)
})

// configure marked once: no mangle/ids (cleaner URLs), GFM tables
marked.setOptions({
  gfm: true,
  breaks: false,
  mangle: false,
  headerIds: false,
})

const html = computed(() => {
  if (!props.source) return ''
  let out = marked.parse(props.source)
  // open external links in a new tab (README links should not hijack the app)
  if (typeof out === 'string') {
    out = out.replace(/<a href="(https?:)/g, '<a target="_blank" rel="noopener" href="$1')
  }
  // 仓库内容是不可信输入：剥离脚本/事件属性/危险标签后再进 v-html
  // （target 加入白名单以保留新窗口语义）
  return DOMPurify.sanitize(out, { ADD_ATTR: ['target'] })
})

// ---- local image inlining ----
// README/markdown reference local images by repo-relative paths (e.g.
// `./assets/screenshot.png`). The Wails webview can't resolve those, so after
// render we swap each relative <img src> for a base64 data URL fetched from
// RepoReadImage (which reads the file out of the current repo).
const host = ref(null)
const backend = window.go?.main?.ByteGitApp

async function resolveLocalImages() {
  if (!host.value || !backend?.RepoReadImage) return
  const imgs = host.value.querySelectorAll('img')
  await Promise.all(Array.from(imgs).map(async (img) => {
    try {
      const raw = img.getAttribute('src') || ''
      // skip remote, already-inlined, protocol-relative, absolute and
      // Windows-drive paths — only resolve repo-relative references.
      if (!raw || /^(https?:|data:|blob:|\/\/)/i.test(raw)) return
      if (raw.startsWith('/') || /^[a-zA-Z]:[\\/]/.test(raw)) return
      // resolve relative to the markdown file's own directory, not the repo
      // root — e.g. gui/README.md + images/x.png → gui/images/x.png
      const baseDir = props.filename ? props.filename.replace(/[^/]+$/, '') : ''
      let rel = baseDir + raw.replace(/^\.\//, '')
      try { rel = decodeURIComponent(rel) } catch {}
      let dataUrl = ''
      if (props.resolveImage) {
        dataUrl = await props.resolveImage(rel)
      } else if (backend?.RepoReadImage) {
        dataUrl = (await backend.RepoReadImage(rel))?.dataUrl || ''
      }
      if (dataUrl) img.src = dataUrl
    } catch { /* leave the original src on any failure */ }
  }))
}

onMounted(() => nextTick(resolveLocalImages))
watch(() => props.source, () => nextTick(resolveLocalImages))
</script>

<template>
  <div ref="host" class="md-view" v-html="html"></div>
</template>

<style scoped>
.md-view {
  font-size: 14px;
  line-height: 1.7;
  color: var(--color-text);
  word-break: break-word;
}
.md-view :deep() h1,
.md-view :deep() h2,
.md-view :deep() h3,
.md-view :deep() h4 {
  font-weight: 600;
  line-height: 1.3;
  margin: 1.4em 0 0.6em;
  color: var(--color-text);
}
.md-view :deep() h1 { font-size: 1.8em; border-bottom: 1px solid var(--color-border-light); padding-bottom: 0.3em; }
.md-view :deep() h2 { font-size: 1.5em; border-bottom: 1px solid var(--color-border-light); padding-bottom: 0.3em; }
.md-view :deep() h3 { font-size: 1.25em; }
.md-view :deep() h4 { font-size: 1.05em; }
.md-view :deep() h1:first-child { margin-top: 0; }

.md-view :deep() p { margin: 0.7em 0; }

.md-view :deep() a {
  color: var(--color-primary);
  text-decoration: none;
}
.md-view :deep() a:hover { text-decoration: underline; }

.md-view :deep() strong { font-weight: 600; }
.md-view :deep() em { font-style: italic; }

.md-view :deep() ul,
.md-view :deep() ol { padding-left: 1.6em; margin: 0.6em 0; }
.md-view :deep() li { margin: 0.25em 0; }

.md-view :deep() code {
  font-family: 'Cascadia Code', 'Fira Code', monospace;
  font-size: 0.88em;
  background: var(--color-surface-raised);
  padding: 2px 6px;
  border-radius: var(--radius-xs);
  color: var(--color-primary);
}
.md-view :deep() pre {
  background: var(--color-background);
  border: 1px solid var(--color-border-light);
  border-radius: var(--radius-sm);
  padding: var(--spacing-sm) var(--spacing-md);
  overflow-x: auto;
  margin: 0.8em 0;
}
.md-view :deep() pre code {
  background: none;
  padding: 0;
  color: var(--color-text);
  font-size: 0.85em;
  line-height: 1.6;
}

.md-view :deep() blockquote {
  border-left: 4px solid var(--color-primary);
  padding: 0.2em 1em;
  margin: 0.8em 0;
  color: var(--color-text-secondary);
  background: var(--color-primary-bg);
  border-radius: 0 var(--radius-xs) var(--radius-xs) 0;
}

.md-view :deep() table {
  border-collapse: collapse;
  margin: 0.8em 0;
  width: 100%;
  font-size: 0.92em;
}
.md-view :deep() th,
.md-view :deep() td {
  border: 1px solid var(--color-border);
  padding: 6px 12px;
  text-align: left;
}
.md-view :deep() th { background: var(--color-surface-raised); font-weight: 600; }
.md-view :deep() tr:nth-child(even) { background: var(--color-background); }

.md-view :deep() hr { border: none; border-top: 1px solid var(--color-border-light); margin: 1.4em 0; }

.md-view :deep() img { max-width: 100%; border-radius: var(--radius-sm); }
</style>
