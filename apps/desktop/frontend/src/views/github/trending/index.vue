<script setup>
import { ref, computed, onMounted, onBeforeUnmount, h } from 'vue'
import { message as AMessage } from 'ant-design-vue'
import {
  GithubOutlined, ReloadOutlined, FireOutlined, StarOutlined,
} from '@ant-design/icons-vue'
import { usePageFooter } from '../../../composables/usePageFooter'
import ContextMenu from '../../../components/common/ContextMenu.vue'
import CtxItem from '../../../components/common/CtxItem.vue'

/**
 * GitHub Trending — 热门仓库浏览（today/week/month + 语言筛选）。
 * 不需要 Token（匿名抓取 github.com/trending）。
 */
const app = window.go?.main?.ByteGitApp
const { setFooter, clearFooter } = usePageFooter()

const projects = ref([])
const languages = ref([])
const spokenLangs = ref([])
const loading = ref(false)
const timeframe = ref('daily') // daily / weekly / monthly
const selectedLang = ref('')   // '' = all programming languages
const selectedSpoken = ref('') // '' = all spoken languages

const timeframeOptions = [
  { label: '今日', value: 'daily' },
  { label: '本周', value: 'weekly' },
  { label: '本月', value: 'monthly' },
]

const langOptions = computed(() => [
  { label: '全部编程语言', value: '' },
  ...languages.value.map((l) => ({ label: l.name, value: l.urlName })),
])

const spokenOptions = computed(() => [
  { label: '全部自然语言', value: '' },
  ...spokenLangs.value.map((l) => ({ label: l.name, value: l.urlName })),
])

async function loadLanguages() {
  if (!app?.GhTrendingLanguages) return
  try { languages.value = (await app.GhTrendingLanguages()) || [] } catch { languages.value = [] }
}

async function loadSpokenLangs() {
  if (!app?.GhSpokenLanguages) return
  try { spokenLangs.value = (await app.GhSpokenLanguages()) || [] } catch { spokenLangs.value = [] }
}

async function loadProjects() {
  if (!app?.GhTrendingProjects) return
  loading.value = true
  try {
    projects.value = (await app.GhTrendingProjects(timeframe.value, selectedLang.value, selectedSpoken.value)) || []
  } catch (e) {
    AMessage.error('加载 Trending 失败: ' + String(e?.message || e))
    projects.value = []
  } finally {
    loading.value = false
  }
}

async function refresh() {
  await Promise.all([loadLanguages(), loadSpokenLangs()])
  await loadProjects()
  updateFooter()
}

function updateFooter() {
  setFooter(TrendingFooter, () => ({
    count: projects.value.length,
    timeframe: timeframeOptions.find((t) => t.value === timeframe.value)?.label || '',
  }))
}

const TrendingFooter = {
  props: ['count', 'timeframe'],
  render() {
    return h('div', { style: 'display:flex;align-items:center;gap:8px;width:100%;font-size:12px;color:var(--color-text-tertiary)' }, [
      h(FireOutlined, { style: 'font-size:13px;color:var(--color-error)' }),
      h('span', { style: 'font-weight:500;color:var(--color-text)' }, 'Trending'),
      h('span', `${this.timeframe} · ${this.count} 个项目`),
    ])
  },
}

function fmtStars(n) {
  if (!n) return '0'
  if (n >= 1000) return (n / 1000).toFixed(1) + 'k'
  return String(n)
}

onMounted(refresh)
onBeforeUnmount(() => clearFooter())

// ---- context menu ----
const ctxMenu = ref({ open: false, x: 0, y: 0, url: '' })

function onCardContextmenu(evt, project) {
  ctxMenu.value = { open: true, x: evt.clientX, y: evt.clientY, url: project.url || '' }
}

function ctxOpenBrowser() {
  ctxMenu.value.open = false
  if (ctxMenu.value.url) app?.OpenInBrowser(ctxMenu.value.url)
}

function ctxCopyUrl() {
  ctxMenu.value.open = false
  if (ctxMenu.value.url) {
    navigator.clipboard.writeText(ctxMenu.value.url)
    AMessage.success('已复制链接', 1.2)
  }
}
</script>

<template>
  <div class="gt-view">
    <!-- toolbar -->
    <div class="gt-toolbar">
      <a-radio-group v-model:value="timeframe" size="small" @change="() => loadProjects()">
        <a-radio-button v-for="t in timeframeOptions" :key="t.value" :value="t.value">{{ t.label }}</a-radio-button>
      </a-radio-group>
      <a-select v-model:value="selectedLang" :options="langOptions" size="small" style="width: 150px" placeholder="编程语言" @change="() => loadProjects()" />
      <a-select v-model:value="selectedSpoken" :options="spokenOptions" size="small" style="width: 140px" placeholder="自然语言" @change="() => loadProjects()" />
      <a-button size="small" type="text" :loading="loading" @click="loadProjects" title="刷新">
        <template #icon><ReloadOutlined /></template>
      </a-button>
    </div>

    <!-- list -->
    <div class="gt-list">
      <div v-if="loading && !projects.length" class="gt-center"><a-spin /></div>
      <div v-else-if="!projects.length" class="gt-empty">
        <FireOutlined style="font-size:36px;opacity:0.2" />
        <div style="margin-top:8px">暂无数据</div>
      </div>
      <div v-for="(p, i) in projects" :key="i" class="gt-card"
        @click="p.url && app?.OpenInBrowser(p.url)"
        @contextmenu.prevent="onCardContextmenu($event, p)"
      >
        <div class="gt-card-rank">#{{ i + 1 }}</div>
        <div class="gt-card-main">
          <div class="gt-card-head">
            <span class="gt-card-name">{{ p.name }}</span>
            <span v-if="p.language" class="gt-card-lang">{{ p.language }}</span>
          </div>
          <div v-if="p.description" class="gt-card-desc">{{ p.description }}</div>
          <div class="gt-card-meta">
            <span class="gt-card-stars"><StarOutlined /> +{{ fmtStars(p.stars) }} stars</span>
          </div>
        </div>
      </div>
    </div>

    <!-- context menu -->
    <ContextMenu v-model:open="ctxMenu.open" :x="ctxMenu.x" :y="ctxMenu.y">
      <CtxItem @click="ctxOpenBrowser"><GithubOutlined /> 在浏览器打开</CtxItem>
      <CtxItem @click="ctxCopyUrl"><StarOutlined /> 复制链接</CtxItem>
    </ContextMenu>
  </div>
</template>

<style scoped>
.gt-view { width: 100%; height: 100%; display: flex; flex-direction: column; background: var(--color-background); padding: var(--spacing-md); overflow: hidden; }

.gt-toolbar { display: flex; align-items: center; gap: 8px; margin-bottom: var(--spacing-sm); flex-shrink: 0; }

.gt-list { flex: 1; overflow-y: auto; display: flex; flex-direction: column; gap: 6px; min-height: 0; }
.gt-center { display: flex; align-items: center; justify-content: center; height: 200px; }
.gt-empty { display: flex; flex-direction: column; align-items: center; justify-content: center; color: var(--color-text-tertiary); font-size: 13px; height: 200px; }

.gt-card {
  display: flex; align-items: flex-start; gap: 10px;
  padding: 10px 14px; border-radius: var(--radius-sm);
  background: var(--color-surface); border: 1px solid var(--color-border);
  cursor: pointer; transition: all var(--transition-fast);
}
.gt-card:hover { border-color: var(--color-primary); box-shadow: var(--shadow-xs); }

.gt-card-rank {
  font-family: 'Cascadia Code', monospace; font-size: 14px; font-weight: 700;
  color: var(--color-text-tertiary); min-width: 32px; text-align: center; flex-shrink: 0;
  padding-top: 2px;
}

.gt-card-main { flex: 1; min-width: 0; }
.gt-card-head { display: flex; align-items: center; gap: 8px; }
.gt-card-name { font-size: 14px; font-weight: 600; color: var(--color-text); }
.gt-card-lang { font-size: 10px; padding: 1px 6px; border-radius: 3px; background: var(--color-info-bg); color: var(--color-info); font-weight: 500; flex-shrink: 0; }
.gt-card-desc { font-size: 12px; color: var(--color-text-secondary); margin-top: 4px; line-height: 1.5; overflow: hidden; text-overflow: ellipsis; display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; }
.gt-card-meta { display: flex; align-items: center; gap: 10px; margin-top: 6px; font-size: 11px; color: var(--color-text-tertiary); }
.gt-card-stars { display: inline-flex; align-items: center; gap: 3px; color: var(--color-warning); font-weight: 500; }
</style>
