<script setup>
import { ref, computed, onMounted, onUnmounted, h, watch, nextTick } from 'vue'
import { message as AMessage } from 'ant-design-vue'
import {
  ExclamationCircleOutlined,
} from '@ant-design/icons-vue'
import { MdPreview } from 'md-editor-v3'
import 'md-editor-v3/lib/preview.css'
import { useThemeStore } from '../../../stores/theme'
import { usePageFooter } from '../../../composables/usePageFooter'
import { fadeIn, staggerIn } from '../../../composables/useGsapAnimations'
import CheatFooter from './components/CheatFooter.vue'

/**
 * Git 备忘录 — 三栏浏览 git-cheat-content。
 *
 *   左：分类（分支管理 / 创建 / ...）
 *   中：该分类下的标题列表
 *   右：选中条目的正文（md-editor-v3 渲染，已去 YAML frontmatter）
 *
 * 首次进入自动 clone 仓库到 ~/.cicbyte/data/git-cheat-content；之后每次
 * 进入静默 pull 更新（失败不打断，用本地旧数据）。
 *
 * 标题 / 统计 / 刷新注入底部全局 footer，最大化阅读区。
 */
const app = window.go?.main?.ByteGitApp
const theme = useThemeStore()
const { setFooter, clearFooter } = usePageFooter()

// ---- data ----
const categories = ref([])
const activeCategory = ref('')
const activeFile = ref('')
const activeEntry = ref(null)
const content = ref('')

// ---- status ----
const phase = ref('loading') // 'loading' | 'ready' | 'error'
const errorMsg = ref('')
const pulling = ref(false)
const contentLoading = ref(false)

// refs for GSAP fade-in of loading / empty / error panels
const loadingRef = ref(null)
const errorRef = ref(null)
const catsRef = ref(null)
const mdRef = ref(null)
// fade-in the loading + error panels when they appear
watch(phase, (p) => {
  if (p === 'loading') nextTick(() => fadeIn(loadingRef.value))
  else if (p === 'error') nextTick(() => fadeIn(errorRef.value))
})
// stagger-in the category list once data is ready
watch(catsRef, (el) => { if (el) nextTick(() => staggerIn(el, '.cv-cat')) })
// fade-in the content panel whenever the selected entry's body finishes loading
// (covers both initial load and switching between entries)
watch(content, (v) => {
  if (v) nextTick(() => fadeIn(mdRef.value))
})

const mdTheme = computed(() => (theme.isDark ? 'dark' : 'light'))

// ---- footer stats (injected into the global footer) ----
const totalEntries = computed(() => categories.value.reduce((s, c) => s + c.entries.length, 0))
const catCount = computed(() => categories.value.length)

const currentCategory = computed(() =>
  categories.value.find((c) => c.name === activeCategory.value) || null,
)
const titles = computed(() => currentCategory.value?.entries || [])

async function bootstrap() {
  phase.value = 'loading'
  errorMsg.value = ''
  try {
    if (!app?.CheatEnsure) throw new Error('后端方法不可用')
    // clone if missing (first use); returns the dir path
    await app.CheatEnsure()
    await loadList()
    // silent background pull — don't block, don't error loudly
    refresh(true)
    phase.value = 'ready'
  } catch (e) {
    phase.value = 'error'
    errorMsg.value = String(e?.message || e)
  }
}

async function loadList() {
  const cats = await app.CheatList()
  categories.value = cats || []
  if (categories.value.length) {
    activeCategory.value = categories.value[0].name
    const first = categories.value[0].entries?.[0]
    if (first) await selectFile(first)
  }
}

async function refresh(silent = false) {
  if (!app?.CheatPull) return
  if (!silent) pulling.value = true
  try {
    await app.CheatPull()
    await loadList()
    if (!silent) AMessage.success('已更新到最新', 1.5)
  } catch {
    if (!silent) AMessage.warning('更新失败，使用本地缓存')
  } finally {
    pulling.value = false
  }
}

function selectCategory(name) {
  activeCategory.value = name
  const first = categories.value.find((c) => c.name === name)?.entries?.[0]
  if (first) selectFile(first)
}

async function selectFile(entry) {
  activeEntry.value = entry
  activeFile.value = entry.file
  content.value = ''
  if (!app?.CheatRead) return
  contentLoading.value = true
  try {
    content.value = await app.CheatRead(entry.file)
  } catch (e) {
    content.value = ''
    AMessage.error('读取失败: ' + String(e?.message || e))
  } finally {
    contentLoading.value = false
  }
}

async function copyCommand() {
  const cmd = activeEntry.value?.command
  if (!cmd) return
  try {
    await navigator.clipboard.writeText(cmd)
    AMessage.success('已复制命令', 1.2)
  } catch {}
}
// expose copyCommand for potential future use (e.g. a copy button inside the
// rendered preview). Currently unused — the md body already shows everything.
void copyCommand

onMounted(() => {
  bootstrap()
  // inject title + stats + refresh into the global footer
  setFooter(CheatFooter, () => ({
    totalEntries: totalEntries.value,
    catCount: catCount.value,
    pulling: pulling.value,
    onRefresh: () => refresh(),
  }))
})
onUnmounted(() => {
  clearFooter()
})
</script>

<template>
  <div class="cv-view">
    <!-- loading / error -->
    <div v-if="phase === 'loading'" ref="loadingRef" class="cv-center">
      <a-spin tip="首次使用，正在下载备忘录内容…" />
    </div>
    <div v-else-if="phase === 'error'" ref="errorRef" class="cv-center">
      <ExclamationCircleOutlined style="font-size:36px;color:var(--color-error);opacity:0.6" />
      <div class="cv-err-title">加载失败</div>
      <div class="cv-err-msg">{{ errorMsg }}</div>
      <a-button type="primary" size="small" @click="bootstrap">重试</a-button>
    </div>

    <!-- three columns -->
    <div v-else class="cv-cols">
      <!-- left: categories -->
      <aside class="cv-cats" ref="catsRef">
        <div
          v-for="c in categories"
          :key="c.name"
          class="cv-cat"
          :class="{ active: c.name === activeCategory }"
          @click="selectCategory(c.name)"
        >
          <span class="cv-cat-name">{{ c.name }}</span>
          <span class="cv-cat-count">{{ c.entries.length }}</span>
        </div>
      </aside>

      <!-- middle: titles -->
      <aside class="cv-titles">
        <div class="cv-titles-head">{{ activeCategory }}</div>
        <div class="cv-titles-body">
          <div
            v-for="t in titles"
            :key="t.file"
            class="cv-title-item"
            :class="{ active: t.file === activeFile }"
            @click="selectFile(t)"
          >
            <div class="cti-name">{{ t.title }}</div>
            <div v-if="t.command" class="cti-cmd">{{ t.command }}</div>
          </div>
        </div>
      </aside>

      <!-- right: content -->
      <section class="cv-content">
        <template v-if="activeEntry">
          <!-- markdown body (title/command/description are all in the md itself) -->
          <div class="cv-md" ref="mdRef">
            <a-spin v-if="contentLoading" class="cv-md-spin" />
            <MdPreview v-else :modelValue="content" :theme="mdTheme" class="cv-md-preview" />
          </div>
        </template>
        <div v-else class="cv-center muted">← 选择左侧条目查看内容</div>
      </section>
    </div>
  </div>
</template>

<style scoped>
.cv-view { width: 100%; height: 100%; display: flex; flex-direction: column; gap: 1px; background: var(--color-border); min-height: 0; }

/* loading / error */
.cv-center { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 12px; color: var(--color-text-tertiary); background: var(--color-surface); min-height: 200px; }
.cv-err-title { font-size: 15px; font-weight: 600; color: var(--color-text); }
.cv-err-msg { font-size: 12px; color: var(--color-text-tertiary); max-width: 400px; text-align: center; word-break: break-all; }
.muted { opacity: 0.6; }

/* three columns */
.cv-cols { flex: 1; display: flex; gap: 1px; min-height: 0; overflow: hidden; }

/* left: categories */
.cv-cats { width: 180px; flex-shrink: 0; background: var(--color-surface); overflow-y: auto; padding: 6px; }
.cv-cat { display: flex; align-items: center; justify-content: space-between; padding: 8px 12px; border-radius: var(--radius-sm); cursor: pointer; color: var(--color-text-secondary); font-size: 13px; transition: all var(--transition-fast); border-left: 2px solid transparent; }
.cv-cat:hover { background: var(--color-hover); color: var(--color-primary); }
.cv-cat.active { background: var(--color-primary-bg); color: var(--color-primary); border-left-color: var(--color-primary); font-weight: 500; }
.cv-cat-count { font-size: 11px; color: var(--color-text-tertiary); background: var(--color-surface-raised); padding: 0 6px; border-radius: 8px; }
.cv-cat.active .cv-cat-count { background: var(--color-primary); color: var(--color-surface); }

/* middle: titles */
.cv-titles { width: 260px; flex-shrink: 0; background: var(--color-background); display: flex; flex-direction: column; min-height: 0; }
.cv-titles-head { padding: 8px 14px; font-size: 12px; font-weight: 600; color: var(--color-text-tertiary); border-bottom: 1px solid var(--color-border-light); flex-shrink: 0; text-transform: uppercase; letter-spacing: 0.03em; }
.cv-titles-body { flex: 1; overflow-y: auto; padding: 4px; }
.cv-title-item { padding: 8px 12px; border-radius: var(--radius-sm); cursor: pointer; transition: all var(--transition-fast); }
.cv-title-item:hover { background: var(--color-hover); }
.cv-title-item.active { background: var(--color-primary-bg); }
.cti-name { font-size: 13px; color: var(--color-text); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.cv-title-item.active .cti-name { color: var(--color-primary); font-weight: 500; }
.cti-cmd { font-family: 'Cascadia Code', monospace; font-size: 10px; color: var(--color-text-tertiary); margin-top: 2px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

/* right: content */
.cv-content { flex: 1; min-width: 0; background: var(--color-surface); display: flex; flex-direction: column; min-height: 0; overflow: hidden; }
.cv-md { flex: 1; min-height: 0; overflow-y: auto; position: relative; }
.cv-md-spin { position: absolute; inset: 0; display: flex; align-items: center; justify-content: center; }
/* md-editor-v3's internal .md-editor-preview has no outer padding by default;
   add comfortable spacing so content doesn't hug the container edges. */
.cv-md :deep(.md-editor) { padding: 0 var(--spacing-lg); background: transparent; }
.cv-md :deep(.md-editor-preview) { padding: var(--spacing-md) 0; }
/* md-editor-v3 ships z-index up to 100001 (code-block header / copy btn),
   which sits above antd modals/drawers (z-index 1000). Clamp every internal
   layer back to a normal stacking context so drawers always win. */
.cv-md :deep(.md-editor) *,
.cv-md :deep(.md-editor-preview *),
.cv-md :deep([class*="md-editor"]) { z-index: auto !important; }
</style>
