<script setup>
import { ref, computed, onMounted, onBeforeUnmount, watch, h } from 'vue'
import { message, Modal } from 'ant-design-vue'
import {
  ApartmentOutlined, ReloadOutlined, PlusOutlined, DeleteOutlined,
  CloudSyncOutlined, GithubOutlined, FolderOpenOutlined, StopOutlined,
  SettingOutlined, ExclamationCircleOutlined, ToolOutlined,
  BranchesOutlined, CalendarOutlined, ReadOutlined, BarChartOutlined,
  CodeOutlined,
} from '@ant-design/icons-vue'
import { useRepoStore } from '../../../stores/repo'
import { useToolStatus } from '../../../composables/useToolStatus'
import { usePageFooter } from '../../../composables/usePageFooter'
import ContextMenu from '../../../components/common/ContextMenu.vue'
import CtxItem from '../../../components/common/CtxItem.vue'
import RefAddModal from '../../../components/reference/RefAddModal.vue'
import RefWikiPanel from '../../../components/reference/RefWikiPanel.vue'
import RefSccPanel from '../../../components/reference/RefSccPanel.vue'
import RefSourcePanel from '../../../components/reference/RefSourcePanel.vue'

const repo = useRepoStore()
const app = window.go?.main?.ByteGitApp
const { reference: refStatus, ensureLoaded } = useToolStatus()
const { setFooter, clearFooter } = usePageFooter()

// ---- data ----
const repos = ref([])        // Repo[]
const mapEntries = ref([])   // MapEntry[] (richer, with topics)
const wikiFiles = ref({})    // { [repoName]: string[] }
const loading = ref(false)
const activeName = ref('')   // selected repo name
const detailTab = ref('source') // 'source' | 'wiki' | 'scc'

// ---- add modal ----
const addOpen = ref(false)
const addLoading = ref(false)

// ---- availability ----
const available = computed(() => refStatus.value.available)

// ---- initialization state ----
// Before listing repos we check whether the project has been initialized
// (.reference/ exists). If not, we show a guided banner instead of an empty
// list — the user needs to init first (creates .reference/, settings, map).
const initialized = ref(false)

async function checkInit() {
  if (!app?.ReferenceSettings) { initialized.value = false; return }
  try {
    const s = await app.ReferenceSettings()
    initialized.value = !!s.hasFile
  } catch {
    initialized.value = false
  }
}

const selectedRepo = computed(() => repos.value.find((r) => r.name === activeName.value))
const selectedMap = computed(() => mapEntries.value.find((m) => m.ref_name === activeName.value))
const selectedTopics = computed(() => selectedMap.value?.topics || [])
const selectedWikiFiles = computed(() => wikiFiles.value[activeName.value] || [])

// =====================================================================
// load
// =====================================================================
async function loadRepos() {
  if (!available.value || !app?.ReferenceList) return
  // skip loading if the project isn't initialized — no .reference/ means
  // reference CLI has nothing to list. The init banner handles this case.
  if (!initialized.value) { repos.value = []; return }
  loading.value = true
  try {
    const res = await app.ReferenceList()
    repos.value = res?.repos || []
    // keep selection valid; default to first
    if (!repos.value.find((r) => r.name === activeName.value)) {
      activeName.value = repos.value[0]?.name || ''
    }
  } catch (e) {
    message.error('加载引用列表失败: ' + String(e?.message || e))
    repos.value = []
  } finally {
    loading.value = false
  }
}

async function loadMap() {
  if (!app?.ReferenceMap) return
  try {
    mapEntries.value = await app.ReferenceMap() || []
  } catch {
    mapEntries.value = []
  }
}

async function loadWikiFiles(name) {
  if (!name || !app?.ReferenceWikiList) return
  try {
    wikiFiles.value[name] = await app.ReferenceWikiList(name) || []
  } catch {
    wikiFiles.value[name] = []
  }
}

async function loadAll() {
  await checkInit()
  if (!initialized.value) return // not initialized — show init banner, skip loading
  await Promise.all([loadRepos(), loadMap()])
  // pre-fetch wiki file lists for all repos
  for (const r of repos.value) loadWikiFiles(r.name)
}

// =====================================================================
// add / remove / update
// =====================================================================
async function onAdd(req) {
  addLoading.value = true
  try {
    await app.ReferenceAdd(req)
    message.success(`已添加引用: ${req.name || req.target}`)
    addOpen.value = false
    await loadAll()
  } catch (e) {
    message.error('添加失败: ' + String(e?.message || e))
  } finally {
    addLoading.value = false
  }
}

function confirmRemove(r) {
  Modal.confirm({
    title: `移除引用「${r.name}」？`,
    icon: undefined,
    content: r.type === 'remote'
      ? '仅从当前项目移除引用（保留全局缓存）。勾选「同时清除缓存」会删除全局缓存仓库。'
      : '仅从当前项目移除引用（本地仓库不受影响）。',
    okText: '移除',
    okType: 'danger',
    cancelText: '取消',
    onOk: () => doRemove(r, false),
  })
}

async function doRemove(r, purge) {
  try {
    await app.ReferenceRemove(r.name, purge)
    message.success(`已移除引用: ${r.name}`)
    if (activeName.value === r.name) activeName.value = ''
    await loadAll()
  } catch (e) {
    message.error('移除失败: ' + String(e?.message || e))
  }
}

const updating = ref(false)
async function doUpdate(name) {
  updating.value = true
  const hide = name ? '' : '（更新全部远程引用，可能耗时较长）'
  try {
    await message.loading({ content: `更新中...${hide}`, key: 'ref-update', duration: 0 })
    await app.ReferenceUpdate(name || '')
    message.success({ content: '更新完成', key: 'ref-update', duration: 1.5 })
    await loadAll()
  } catch (e) {
    message.error({ content: '更新失败: ' + String(e?.message || e), key: 'ref-update', duration: 3 })
  } finally {
    updating.value = false
  }
}
async function cancelUpdate() {
  try { if (app?.CancelTask) await app.CancelTask() } catch {}
  message.destroy('ref-update')
  updating.value = false
  message.info('更新已取消')
}

// ---- per-item context menu (right-click on a repo card) ----
// carries the clicked repo + cursor position; entries reuse the same
// confirmRemove / doUpdate handlers that the toolbar buttons used.
const ctxMenu = ref({ open: false, x: 0, y: 0, repo: null })

function onItemContextmenu(evt, r) {
  // selecting on right-click matches the click behaviour and ensures the
  // menu actions target the visible repo (footer + tabs follow selection)
  activeName.value = r.name
  ctxMenu.value = { open: true, x: evt.clientX, y: evt.clientY, repo: r }
}

function ctxUpdate() {
  const r = ctxMenu.value.repo
  ctxMenu.value.open = false
  if (r) doUpdate(r.name)
}
function ctxRemove() {
  const r = ctxMenu.value.repo
  ctxMenu.value.open = false
  if (r) confirmRemove(r)
}

// =====================================================================
// lifecycle
// =====================================================================
onMounted(async () => {
  await ensureLoaded()
  await loadAll()
})
onBeforeUnmount(() => clearFooter())

watch(() => repo.path, () => { loadAll() })
watch(() => available.value, (v) => { if (v) loadAll() })
watch(activeName, (n) => { if (n) loadWikiFiles(n); updateFooter() })
watch([repos, mapEntries], updateFooter, { deep: false })

function sourceIcon(r) {
  return r.type === 'local' ? FolderOpenOutlined : GithubOutlined
}
function shortHash(h) { return (h || '').slice(0, 7) }

// inline-render global footer: surfaces the currently-selected repo's info
// (name · type · branch · commit · source) so the page can drop its own
// header bar and reclaim vertical space for the file tree / code view.
// Mirrors the GhFooter pattern in GitHubView.vue.
function updateFooter() {
  if (!available.value || !selectedRepo.value) { clearFooter(); return }
  setFooter(RefFooter, () => ({
    name: selectedRepo.value.name,
    type: selectedRepo.value.type,
    source: selectedRepo.value.source,
    branch: selectedRepo.value.branch,
    commitAt: selectedRepo.value.commit_at,
    commitHash: selectedMap.value?.commit || '',
    topics: (selectedMap.value?.topics || []).length,
    total: repos.value.length,
  }))
}

const RefFooter = {
  props: ['name', 'type', 'source', 'branch', 'commitAt', 'commitHash', 'topics', 'total'],
  render() {
    return h('div', { style: 'display:flex;align-items:center;gap:10px;width:100%;font-size:12px;color:var(--color-text-tertiary)' }, [
      h(ApartmentOutlined, { style: 'font-size:13px;color:var(--color-primary)' }),
      h('span', { style: 'font-weight:600;color:var(--color-text)' }, this.name),
      h('span', { style: 'color:var(--color-text-tertiary)' }, this.type === 'local' ? '本地' : '远程'),
      this.branch ? h('span', [h(BranchesOutlined), ` ${this.branch}`]) : null,
      this.commitAt ? h('span', [h(CalendarOutlined), ` ${this.commitAt}`]) : null,
      this.commitHash ? h('span', { style: 'font-family:Cascadia Code,monospace' }, `#${shortHash(this.commitHash)}`) : null,
      this.topics ? h('span', [h(ReadOutlined), ` ${this.topics} 主题`]) : null,
      h('code', { style: 'font-size:10px;color:var(--color-text-tertiary);overflow:hidden;text-overflow:ellipsis;white-space:nowrap', title: this.source }, this.source),
      h('div', { style: 'flex:1' }),
      h('span', `${this.total} 个引用`),
    ])
  },
}
</script>

<template>
  <div class="ref-view">
    <!-- not available banner -->
    <div v-if="!available" class="rv-na">
      <ExclamationCircleOutlined class="na-icon" />
      <div class="na-body">
        <div class="na-title">reference CLI 未就绪</div>
        <div class="na-msg">{{ refStatus.message || '请先安装或在设置中配置 reference.exe 路径' }}</div>
      </div>
      <router-link to="/settings" class="na-btn">
        <SettingOutlined /> 去配置
      </router-link>
    </div>

    <!-- not initialized banner -->
    <div v-else-if="!initialized" class="rv-na rv-init">
      <ApartmentOutlined class="na-icon" style="color: var(--color-primary)" />
      <div class="na-body">
        <div class="na-title">当前项目尚未初始化引用</div>
        <div class="na-msg">初始化会创建 <code>.reference/</code> 目录结构、生成配置文件。完成后即可添加和管理代码引用。</div>
      </div>
      <router-link to="/reference-ops" class="na-btn">
        <ToolOutlined /> 去初始化
      </router-link>
    </div>

    <!-- main split: list + detail -->
    <div v-else class="rv-main">
      <!-- left: repo list with integrated toolbar header -->
      <aside class="rv-list">
        <div class="rl-head">
          <span class="rl-title"><ApartmentOutlined /> 引用 <span class="rl-count">{{ repos.length }}</span></span>
          <div class="rl-tools">
            <a-button v-if="updating" size="small" type="text" danger @click="cancelUpdate" title="取消更新">
              <template #icon><StopOutlined /></template>
            </a-button>
            <a-button v-else size="small" type="text" :disabled="!repos.length" @click="doUpdate('')" title="更新全部远程引用缓存">
              <template #icon><CloudSyncOutlined /></template>
            </a-button>
            <a-button size="small" type="text" :loading="loading" @click="loadAll" title="刷新列表">
              <template #icon><ReloadOutlined /></template>
            </a-button>
            <a-button size="small" type="text" @click="addOpen = true" title="添加引用">
              <template #icon><PlusOutlined /></template>
            </a-button>
          </div>
        </div>
        <div v-if="loading && !repos.length" class="rl-loading">
          <a-spin tip="加载引用..." />
        </div>
        <div v-else-if="!repos.length" class="rl-empty">
          <ApartmentOutlined class="re-icon" />
          <div class="re-title">暂无引用</div>
          <div class="re-desc">点击上方 + 添加</div>
        </div>
        <div v-else class="rl-items">
          <div
            v-for="r in repos" :key="r.name"
            class="rl-item"
            :class="{ active: activeName === r.name, local: r.type === 'local' }"
            @click="activeName = r.name"
            @contextmenu.prevent="onItemContextmenu($event, r)"
          >
            <div class="rli-head">
              <component :is="sourceIcon(r)" class="rli-source" />
              <span class="rli-name">{{ r.name }}</span>
              <a-tag v-if="r.type === 'local'" color="blue" class="rli-tag">本地</a-tag>
              <a-tag v-else color="default" class="rli-tag">远程</a-tag>
            </div>
            <div class="rli-source-txt" :title="r.source">{{ r.source }}</div>
            <div class="rli-meta">
              <span v-if="r.branch" class="rlm-item"><BranchesOutlined /> {{ r.branch }}</span>
              <span v-if="r.commit_at" class="rlm-item"><CalendarOutlined /> {{ r.commit_at }}</span>
              <span class="rlm-item" v-if="mapEntries.find((m) => m.ref_name === r.name)?.topics?.length">
                <ReadOutlined /> {{ mapEntries.find((m) => m.ref_name === r.name).topics.length }} 主题
              </span>
            </div>
          </div>
        </div>
      </aside>

      <!-- right: detail (no header bar — info lives in the global footer) -->
      <div class="rv-detail">
        <div v-if="!selectedRepo" class="rd-empty">
          <ApartmentOutlined class="rd-empty-icon" />
          <div>从左侧选择一个引用查看详情</div>
        </div>
        <template v-else>
          <!-- tab nav (per-repo actions live in the list-item context menu) -->
          <div class="rd-tabs">
            <button class="rd-tab" :class="{ on: detailTab === 'source' }" @click="detailTab = 'source'"><CodeOutlined /> 源码</button>
            <button class="rd-tab" :class="{ on: detailTab === 'wiki' }" @click="detailTab = 'wiki'"><ReadOutlined /> 知识库</button>
            <button class="rd-tab" :class="{ on: detailTab === 'scc' }" @click="detailTab = 'scc'"><BarChartOutlined /> 代码统计</button>
          </div>

          <!-- tab content -->
          <div class="rd-body">
            <RefSourcePanel
              v-if="detailTab === 'source'"
              :repo-name="activeName"
            />
            <RefWikiPanel
              v-else-if="detailTab === 'wiki'"
              :repo-name="activeName"
              :topics="selectedTopics"
              :wiki-files="selectedWikiFiles"
            />
            <RefSccPanel
              v-else
              :repo-name="activeName"
            />
          </div>
        </template>
      </div>
    </div>

    <!-- add modal -->
    <RefAddModal v-model:open="addOpen" :loading="addLoading" @submit="onAdd" />

    <!-- per-item context menu (right-click a repo card) -->
    <ContextMenu v-model:open="ctxMenu.open" :x="ctxMenu.x" :y="ctxMenu.y" :title="ctxMenu.repo?.name">
      <template v-if="ctxMenu.repo && ctxMenu.repo.type !== 'local'">
        <CtxItem v-if="updating" @click="cancelUpdate">
          <StopOutlined /> 取消更新
        </CtxItem>
        <CtxItem v-else @click="ctxUpdate">
          <CloudSyncOutlined /> 更新缓存
        </CtxItem>
        <div class="ctx-sep"></div>
      </template>
      <CtxItem danger @click="ctxRemove">
        <DeleteOutlined /> 移除引用
      </CtxItem>
    </ContextMenu>
  </div>
</template>

<style scoped>
.ref-view { width: 100%; height: 100%; display: flex; flex-direction: column; min-height: 0; }

/* not available */
.rv-na {
  margin: 24px; padding: 18px 22px;
  display: flex; align-items: center; gap: 14px;
  background: var(--color-warning-bg, rgba(250, 173, 20, 0.1));
  border: 1px solid var(--color-warning, #faad14);
  border-radius: var(--radius-md);
}
.rv-init {
  background: var(--color-primary-bg, rgba(125, 108, 248, 0.08));
  border-color: var(--color-primary);
}
.na-icon { font-size: 28px; color: var(--color-warning, #faad14); }
.na-body { flex: 1; }
.na-title { font-size: 14px; font-weight: 600; color: var(--color-text); }
.na-msg { font-size: 12px; color: var(--color-text-secondary); margin-top: 4px; white-space: pre-wrap; }
.na-btn {
  display: inline-flex; align-items: center; gap: 6px;
  padding: 6px 14px; border-radius: var(--radius-sm);
  background: var(--color-primary); color: #fff; font-size: 12px;
}
.na-btn:hover { opacity: 0.9; color: #fff; }

/* main split — no top header bar; info lives in the global footer */
.rv-main { flex: 1; display: flex; min-height: 0; gap: 1px; background: var(--color-border-light); }

/* left list */
.rv-list { width: 280px; flex-shrink: 0; background: var(--color-surface); display: flex; flex-direction: column; overflow: hidden; }
.rl-head {
  display: flex; align-items: center; justify-content: space-between;
  padding: 8px 10px; border-bottom: 1px solid var(--color-border-light); flex-shrink: 0;
}
.rl-title { display: inline-flex; align-items: center; gap: 6px; font-size: 12px; font-weight: 600; color: var(--color-text-secondary); text-transform: uppercase; letter-spacing: 0.04em; }
.rl-title :deep(.anticon) { color: var(--color-primary); font-size: 14px; }
.rl-count { font-size: 10px; font-weight: 500; color: var(--color-text-tertiary); background: var(--color-background); padding: 1px 6px; border-radius: 999px; }
.rl-tools { display: flex; align-items: center; gap: 4px; }
.rl-loading, .rl-empty { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 10px; color: var(--color-text-tertiary); font-size: 13px; }
.re-icon { font-size: 36px; opacity: 0.4; }
.re-title { font-size: 14px; font-weight: 600; color: var(--color-text-secondary); }
.re-desc { font-size: 12px; color: var(--color-text-tertiary); }
.rl-items { flex: 1; overflow-y: auto; padding: 8px; display: flex; flex-direction: column; gap: 4px; }
.rl-item {
  padding: 10px 12px; border-radius: var(--radius-sm); cursor: pointer;
  border: 1px solid transparent; transition: all var(--transition-fast);
}
.rl-item:hover { background: var(--color-hover); }
.rl-item.active { background: var(--color-primary-bg); border-color: var(--color-primary); }
.rli-head { display: flex; align-items: center; gap: 6px; }
.rli-source { font-size: 13px; color: var(--color-text-tertiary); }
.rl-item.active .rli-source { color: var(--color-primary); }
.rli-name { font-size: 13px; font-weight: 600; color: var(--color-text); flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.rl-item.active .rli-name { color: var(--color-primary); }
.rli-tag { margin: 0; transform: scale(0.85); transform-origin: right; }
.rli-source-txt {
  font-size: 11px; color: var(--color-text-tertiary);
  margin-top: 4px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  font-family: 'Cascadia Code', monospace;
}
.rli-meta { display: flex; gap: 8px; margin-top: 5px; font-size: 10px; color: var(--color-text-tertiary); flex-wrap: wrap; }
.rlm-item { display: inline-flex; align-items: center; gap: 3px; white-space: nowrap; }
.rlm-item :deep(.anticon) { font-size: 11px; }

/* right detail — no header bar; tabs + body only */
.rv-detail { flex: 1; min-width: 0; background: var(--color-surface); display: flex; flex-direction: column; overflow: hidden; }
.rd-empty { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 12px; color: var(--color-text-tertiary); font-size: 13px; }
.rd-empty-icon { font-size: 48px; opacity: 0.3; }

/* tabs row (per-repo actions live in the list-item context menu) */
.rd-tabs {
  display: flex; gap: 2px;
  padding: 0 8px 0 12px; border-bottom: 1px solid var(--color-border-light); flex-shrink: 0;
}
.rd-tab {
  padding: 8px 12px; cursor: pointer; font-size: 13px;
  border: none; background: transparent; color: var(--color-text-secondary);
  border-bottom: 2px solid transparent; transition: all var(--transition-fast);
  display: inline-flex; align-items: center; gap: 6px;
}
.rd-tab:hover { color: var(--color-primary); }
.rd-tab.on { color: var(--color-primary); border-bottom-color: var(--color-primary); font-weight: 600; }

/* body */
.rd-body { flex: 1; min-height: 0; overflow: hidden; }
</style>
