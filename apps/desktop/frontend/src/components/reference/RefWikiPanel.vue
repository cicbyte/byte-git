<script setup>
/**
 * Wiki 浏览面板 — 展示某个引用仓库的知识文件（reference.md + 主题 .md）。
 * 左侧主题列表，右侧 markdown 渲染。
 *
 * 数据来源：父组件传入 topics（来自 reference.map.jsonl）+ wikiFiles（目录扫描），
 * 文件内容通过 ReferenceWikiFile(name, file) 读取。
 */
import { ref, watch, computed } from 'vue'
import { FileTextOutlined, ReadOutlined, ReloadOutlined } from '@ant-design/icons-vue'
import { MdPreview } from 'md-editor-v3'
import 'md-editor-v3/lib/preview.css'
import { useThemeStore } from '../../stores/theme'

const props = defineProps({
  repoName: { type: String, default: '' },
  // from map.jsonl: [{ file, description, commit }]
  topics: { type: Array, default: () => [] },
  // from directory scan: string[] (may overlap with topics)
  wikiFiles: { type: Array, default: () => [] },
})

const app = window.go?.main?.ByteGitApp
const theme = useThemeStore()
const mdTheme = computed(() => (theme.isDark ? 'dark' : 'light'))

const selected = ref('')
const content = ref('')
const loading = ref(false)
const error = ref('')

// merged + deduped file list. reference.md is pinned first if present.
const files = computed(() => {
  const set = new Map()
  for (const f of props.wikiFiles) set.set(f, { file: f, description: '', commit: '' })
  for (const t of props.topics) {
    if (!set.has(t.file)) set.set(t.file, { file: t.file, description: t.description || '', commit: t.commit || '' })
    else set.get(t.file).description = set.get(t.file).description || t.description || ''
  }
  const arr = [...set.values()]
  // pin reference.md first
  arr.sort((a, b) => {
    if (a.file === 'reference.md') return -1
    if (b.file === 'reference.md') return 1
    return a.file.localeCompare(b.file)
  })
  return arr
})

const descriptionOf = (file) => {
  if (file === 'reference.md') return '项目知识总览（架构、设计决策）'
  const t = props.topics.find((x) => x.file === file)
  return t?.description || ''
}

async function openFile(file) {
  if (!file || !app?.ReferenceWikiFile) return
  selected.value = file
  loading.value = true
  error.value = ''
  content.value = ''
  try {
    content.value = await app.ReferenceWikiFile(props.repoName, file)
  } catch (e) {
    error.value = String(e?.message || e)
  } finally {
    loading.value = false
  }
}

// auto-open reference.md on repo change
watch(
  () => props.repoName,
  () => {
    const ref = files.value.find((f) => f.file === 'reference.md')
    openFile(ref ? 'reference.md' : (files.value[0]?.file || ''))
  },
  { immediate: true },
)
</script>

<template>
  <div class="wiki-panel">
    <!-- left: file list -->
    <aside class="wiki-list">
      <div class="wl-head">
        <ReadOutlined class="wl-icon" />
        <span>知识文件</span>
        <span class="wl-count">{{ files.length }}</span>
      </div>
      <div class="wl-items">
        <div
          v-for="f in files" :key="f.file"
          class="wl-item"
          :class="{ active: selected === f.file }"
          @click="openFile(f.file)"
        >
          <FileTextOutlined class="wli-icon" />
          <div class="wli-body">
            <div class="wli-name">{{ f.file }}</div>
            <div class="wli-desc" v-if="descriptionOf(f.file)">{{ descriptionOf(f.file) }}</div>
          </div>
        </div>
        <div v-if="!files.length" class="wl-empty">暂无知识文件</div>
      </div>
    </aside>

    <!-- right: markdown preview -->
    <div class="wiki-content">
      <div v-if="loading" class="wc-loading">
        <a-spin tip="加载中..." />
      </div>
      <div v-else-if="error" class="wc-error">{{ error }}</div>
      <div v-else-if="!selected" class="wc-empty">
        <FileTextOutlined class="wc-empty-icon" />
        <div>从左侧选择一个文件查看内容</div>
      </div>
      <div v-else class="wc-md">
        <MdPreview
          :model-value="content"
          :theme="mdTheme"
        />
      </div>
    </div>
  </div>
</template>

<style scoped>
.wiki-panel { display: flex; height: 100%; min-height: 0; gap: 1px; background: var(--color-border-light); }

/* left list */
.wiki-list {
  width: 240px; flex-shrink: 0;
  background: var(--color-surface);
  display: flex; flex-direction: column;
  overflow: hidden;
}
.wl-head {
  display: flex; align-items: center; gap: 8px;
  padding: 12px 14px; border-bottom: 1px solid var(--color-border-light);
  font-size: 13px; font-weight: 600; color: var(--color-text);
}
.wl-icon { color: var(--color-primary); }
.wl-count {
  font-size: 11px; font-weight: 500; color: var(--color-text-tertiary);
  background: var(--color-background); padding: 1px 7px; border-radius: 999px;
}
.wl-items { flex: 1; overflow-y: auto; padding: 6px; }
.wl-item {
  display: flex; gap: 8px; padding: 8px 10px;
  border-radius: var(--radius-sm); cursor: pointer;
  transition: background var(--transition-fast);
}
.wl-item:hover { background: var(--color-hover); }
.wl-item.active { background: var(--color-primary-bg); }
.wli-icon { font-size: 14px; color: var(--color-text-tertiary); flex-shrink: 0; margin-top: 2px; }
.wl-item.active .wli-icon { color: var(--color-primary); }
.wli-body { flex: 1; min-width: 0; }
.wli-name {
  font-size: 12px; color: var(--color-text);
  overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
}
.wl-item.active .wli-name { color: var(--color-primary); font-weight: 500; }
.wli-desc {
  font-size: 11px; color: var(--color-text-tertiary);
  margin-top: 2px; line-height: 1.4;
  overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
}
.wl-empty { padding: 20px 10px; font-size: 12px; color: var(--color-text-tertiary); text-align: center; }

/* right content */
.wiki-content {
  flex: 1; min-width: 0; background: var(--color-surface);
  display: flex; flex-direction: column; overflow: hidden;
}
.wc-loading, .wc-empty, .wc-error {
  flex: 1; display: flex; flex-direction: column;
  align-items: center; justify-content: center; gap: 12px;
  color: var(--color-text-tertiary); font-size: 13px;
}
.wc-empty-icon { font-size: 36px; opacity: 0.4; }
.wc-error { color: var(--color-error); }
.wc-md { flex: 1; overflow-y: auto; background: var(--color-surface); }

/* md-editor-v3's internal .md-editor-preview has no outer padding by default,
 * leaving content flush against the panel edges. Inject padding to match the
 * surrounding card chrome. Same fix as CheatView's .cv-md. */
.wc-md :deep(.md-editor) { padding: 0 var(--spacing-lg); background: transparent; }
.wc-md :deep(.md-editor-preview) { padding: var(--spacing-md) 0; }
/* md-editor-v3 ships z-index up to 100001 (code-block header / copy btn),
 * which sits above our sidebar/header. Clamp to keep it within the panel. */
.wc-md :deep(.md-editor) * { z-index: auto !important; }
</style>
