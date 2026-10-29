<script setup>
import { ref, computed, onMounted, watch, onBeforeUnmount, h } from 'vue'
import { useRouter } from 'vue-router'
import { message as AMessage, Modal } from 'ant-design-vue'
import {
  GithubOutlined, ReloadOutlined, BranchesOutlined,
  CheckOutlined, CheckCircleOutlined, ExclamationCircleOutlined,
  PlusOutlined, CommentOutlined, FileTextOutlined,
} from '@ant-design/icons-vue'
import { useRepoStore } from '@/stores/repo'
import { usePageFooter } from '@/composables/usePageFooter'
import { useGitHubCache } from '@/composables/useGitHub'
import MarkdownView from '@/components/common/MarkdownView.vue'
import { MdEditor } from 'md-editor-v3'
import 'md-editor-v3/lib/style.css'
import { useThemeStore } from '@/stores/theme'

const app = window.go?.main?.ByteGitApp
const repo = useRepoStore()
const router = useRouter()
const { status, prs, ensureStatusLoaded, refreshStatus, invalidatePRs, ensurePRsLoaded, refreshPRs, invalidateAll } = useGitHubCache()
const { setFooter, clearFooter } = usePageFooter()
const themeStore = useThemeStore()
const mdTheme = computed(() => (themeStore.isDark ? 'dark' : 'light'))

const loading = ref(false)
const acting = ref(false)
const activePR = ref(null)
const prState = ref('open') // 'open' / 'closed' / 'all'
const searchQuery = ref('')

// detail panel state
const detailTab = ref('files') // 'files' | 'comments'
const prFiles = ref([])
const prComments = ref([])
const detailLoading = ref(false)

// modals
const createOpen = ref(false)
const createForm = ref({ title: '', head: '', base: 'main', body: '' })
const creating = ref(false)
const commentOpen = ref(false)
const commentBody = ref('')
const commenting = ref(false)
const reviewOpen = ref(false)
const reviewEvent = ref('COMMENT')
const reviewBody = ref('')
const reviewing = ref(false)

const canUse = computed(() => status.value.authed && status.value.remoteIsGh)

// client-side search filter
const filteredPRs = computed(() => {
  const q = searchQuery.value.trim().toLowerCase()
  if (!q) return prs.value
  return prs.value.filter((p) =>
    p.title.toLowerCase().includes(q) ||
    p.author.toLowerCase().includes(q) ||
    p.headRef.toLowerCase().includes(q) ||
    String(p.number).includes(q),
  )
})

watch(prs, (v) => { if (v.length && !activePR.value) activePR.value = v[0] }, { immediate: true })

watch(activePR, (pr) => {
  prFiles.value = []
  prComments.value = []
  if (pr) loadDetail(pr.number)
}, { immediate: false })

async function loadPRs() {
  if (!canUse.value) return
  loading.value = true
  invalidatePRs()
  try {
    prs.value = (await app.GhPRList(prState.value)) || []
  } catch (e) {
    AMessage.error('加载 PR 失败: ' + String(e?.message || e))
    prs.value = []
  } finally {
    loading.value = false
  }
  if (prs.value.length) activePR.value = prs.value[0]
  updateFooter()
}

async function loadDetail(number) {
  detailLoading.value = true
  try {
    const [files, comments] = await Promise.all([
      app.GhPRFiles(number).catch(() => []),
      app.GhPRComments(number).catch(() => []),
    ])
    prFiles.value = files || []
    prComments.value = comments || []
  } catch {} finally { detailLoading.value = false }
}

function updateFooter() {
  if (!canUse.value) return
  setFooter(GhFooter, () => ({
    user: status.value.user,
    repo: status.value.ownerRepo,
    prCount: prs.value.length,
  }))
}

const GhFooter = {
  props: ['user', 'repo', 'prCount'],
  render() {
    return h('div', { style: 'display:flex;align-items:center;gap:8px;width:100%;font-size:12px;color:var(--color-text-tertiary)' }, [
      h(GithubOutlined, { style: 'font-size:13px' }),
      h('span', { style: 'font-weight:500;color:var(--color-text)' }, 'GitHub'),
      h('span', `${this.user} · ${this.repo}`),
      h('div', { style: 'flex:1' }),
      h('span', `${this.prCount} 个 PR`),
    ])
  },
}

function checkoutPR(pr) {
  Modal.confirm({
    title: `Checkout PR #${pr.number}?`, content: `将切换到分支 ${pr.headRef}`,
    okText: 'Checkout', cancelText: '取消',
    async onOk() {
      acting.value = true
      try { await app.GhPRCheckout(pr.number); AMessage.success(`已切换到 PR #${pr.number}`); repo.detectCurrent() }
      catch (e) { AMessage.error('Checkout 失败: ' + String(e?.message || e)) }
      finally { acting.value = false }
    },
  })
}

function mergePR(pr) {
  Modal.confirm({
    title: `Merge PR #${pr.number}?`, content: `${pr.title}\n将合并 ${pr.headRef} → ${pr.baseRef}`,
    okText: 'Merge', cancelText: '取消',
    async onOk() {
      acting.value = true
      try { await app.GhPRMerge(pr.number, 'merge'); AMessage.success(`已合并 PR #${pr.number}`); await loadPRs() }
      catch (e) { AMessage.error('Merge 失败: ' + String(e?.message || e)) }
      finally { acting.value = false }
    },
  })
}

// ---- create PR ----
function openCreate() {
  createForm.value = { title: '', head: repo.branch || '', base: 'main', body: '' }
  createOpen.value = true
}

async function doCreate() {
  if (!createForm.value.title.trim() || !createForm.value.head.trim()) { AMessage.warning('请填写标题和源分支'); return }
  creating.value = true
  try {
    const pr = await app.GhPRCreate(createForm.value.title.trim(), createForm.value.head.trim(), createForm.value.base.trim(), createForm.value.body)
    AMessage.success(`已创建 PR #${pr.number}`)
    createOpen.value = false
    invalidatePRs()
    await loadPRs()
  } catch (e) { AMessage.error('创建失败: ' + String(e?.message || e)) }
  finally { creating.value = false }
}

// ---- comment ----
function openComment() { commentBody.value = ''; commentOpen.value = true }

async function doComment() {
  if (!commentBody.value.trim()) { AMessage.warning('请输入评论'); return }
  commenting.value = true
  try {
    await app.GhPRCreateComment(activePR.value.number, commentBody.value)
    AMessage.success('评论已发送')
    commentOpen.value = false
    await loadDetail(activePR.value.number)
  } catch (e) { AMessage.error('评论失败: ' + String(e?.message || e)) }
  finally { commenting.value = false }
}

// ---- review ----
function openReview(event) { reviewEvent.value = event; reviewBody.value = ''; reviewOpen.value = true }

async function doReview() {
  reviewing.value = true
  try {
    await app.GhPRReview(activePR.value.number, reviewEvent.value, reviewBody.value)
    AMessage.success(reviewEvent.value === 'APPROVE' ? '已批准' : reviewEvent.value === 'REQUEST_CHANGES' ? '已请求修改' : '评论已提交')
    reviewOpen.value = false
  } catch (e) { AMessage.error('操作失败: ' + String(e?.message || e)) }
  finally { reviewing.value = false }
}

onMounted(async () => {
  await ensureStatusLoaded()
  if (canUse.value) {
    await ensurePRsLoaded()
    updateFooter()
  }
})
onBeforeUnmount(() => clearFooter())

watch(() => repo.repoEpoch, () => { invalidateAll(); loadPRs() })
</script>

<template>
  <div class="gh-view">
    <!-- guides (same as before) -->
    <div v-if="!status.tokenSet" class="gh-guide">
      <GithubOutlined class="gh-guide-icon" />
      <div class="gh-guide-title">需要配置 GitHub Token</div>
      <a-button type="primary" size="small" @click="router.push('/settings')">前往设置</a-button>
    </div>
    <div v-else-if="!status.authed" class="gh-guide">
      <ExclamationCircleOutlined class="gh-guide-icon warn" />
      <div class="gh-guide-title">Token 无效</div>
      <a-button type="primary" size="small" @click="router.push('/settings')">前往设置</a-button>
    </div>
    <div v-else-if="!status.remoteIsGh" class="gh-guide">
      <ExclamationCircleOutlined class="gh-guide-icon warn" />
      <div class="gh-guide-title">当前仓库不是 GitHub 仓库</div>
    </div>

    <!-- ready: two-pane layout -->
    <div v-else class="gh-ready">
      <!-- left: PR list -->
      <aside class="gh-list-pane">
        <div class="gh-head">
          <span class="gh-repo">{{ status.ownerRepo }}</span>
          <div class="gh-head-btns">
            <a-button size="small" @click="openCreate" title="创建 PR"><PlusOutlined /></a-button>
          </div>
        </div>
        <!-- filter bar: two rows to avoid crowding in 300px pane -->
        <div class="gh-filter-bar">
          <a-radio-group v-model:value="prState" size="small" @change="() => loadPRs()">
            <a-radio-button value="open">Open</a-radio-button>
            <a-radio-button value="closed">Closed</a-radio-button>
            <a-radio-button value="all">All</a-radio-button>
          </a-radio-group>
          <a-button size="small" type="text" :loading="loading" @click="loadPRs" title="刷新">
            <template #icon><ReloadOutlined /></template>
          </a-button>
        </div>
        <div class="gh-filter-bar gh-filter-bar-2">
          <a-input v-model:value="searchQuery" placeholder="搜索标题/作者/分支…" size="small" allow-clear />
        </div>
        <div class="gh-pr-list">
          <div v-if="loading && !prs.length" class="gh-center"><a-spin /></div>
          <div v-else-if="!filteredPRs.length" class="gh-empty"><CheckCircleOutlined style="font-size:28px;opacity:0.4" /><div>{{ searchQuery ? '无匹配结果' : '没有 PR' }}</div></div>
          <div v-for="pr in filteredPRs" :key="pr.number" class="gh-pr-card" :class="{ active: activePR?.number === pr.number }" @click="activePR = pr">
            <div class="gh-pr-main">
              <div class="gh-pr-title-row">
                <span class="gh-pr-num">#{{ pr.number }}</span>
                <span class="gh-pr-title">{{ pr.title }}</span>
                <span v-if="pr.state === 'closed'" class="gh-pr-state-closed">closed</span>
              </div>
              <div class="gh-pr-meta">
                <span class="gh-pr-branch"><BranchesOutlined /> {{ pr.headRef }} → {{ pr.baseRef }}</span>
                <span class="gh-pr-author">{{ pr.author }}</span>
                <span v-if="pr.isDraft" class="gh-pr-draft">Draft</span>
              </div>
            </div>
          </div>
        </div>
      </aside>

      <!-- right: PR detail -->
      <section class="gh-detail-pane">
        <template v-if="activePR">
          <!-- detail head -->
          <div class="gh-detail-head">
            <span class="gh-detail-title">#{{ activePR.number }} {{ activePR.title }}</span>
            <div class="gh-detail-actions">
              <a-button size="small" type="text" @click="checkoutPR(activePR)" title="Checkout"><BranchesOutlined /></a-button>
              <a-button size="small" type="text" @click="openComment" title="评论"><CommentOutlined /></a-button>
              <a-button size="small" type="text" @click="openReview('APPROVE')" title="批准">✓</a-button>
              <a-button size="small" type="text" @click="mergePR(activePR)" title="Merge"><CheckOutlined /></a-button>
            </div>
          </div>

          <!-- body -->
          <div v-if="activePR.body" class="gh-detail-body-preview">
            <MarkdownView :source="activePR.body" />
          </div>

          <!-- tabs -->
          <div class="gh-detail-tabs">
            <button class="gh-tab" :class="{ active: detailTab === 'files' }" @click="detailTab = 'files'">
              <FileTextOutlined /> 文件 ({{ prFiles.length }})
            </button>
            <button class="gh-tab" :class="{ active: detailTab === 'comments' }" @click="detailTab = 'comments'">
              <CommentOutlined /> 评论 ({{ prComments.length }})
            </button>
          </div>

          <div class="gh-detail-body">
            <a-spin v-if="detailLoading" size="small" class="gh-detail-spin" />
            <!-- files tab -->
            <template v-else-if="detailTab === 'files'">
              <div v-if="!prFiles.length" class="gh-detail-empty">无文件变更</div>
              <div v-for="f in prFiles" :key="f.filename" class="gh-file-item">
                <div class="gh-file-head">
                  <span class="gh-file-status" :class="'fs-' + f.status">{{ f.status[0].toUpperCase() }}</span>
                  <span class="gh-file-name" :title="f.filename">{{ f.filename }}</span>
                  <span class="gh-file-delta"><span style="color:var(--color-success)">+{{ f.additions }}</span> <span style="color:var(--color-error)">-{{ f.deletions }}</span></span>
                </div>
                <pre v-if="f.patch" class="gh-file-patch">{{ f.patch }}</pre>
              </div>
            </template>
            <!-- comments tab -->
            <template v-else>
              <div v-if="!prComments.length" class="gh-detail-empty">无评论</div>
              <div v-for="c in prComments" :key="c.id" class="gh-comment-item">
                <div class="gh-comment-head">
                  <span class="gh-comment-author">@{{ c.author }}</span>
                  <span v-if="c.path" class="gh-comment-path">{{ c.path }}{{ c.line ? ':' + c.line : '' }}</span>
                  <span class="gh-comment-date">{{ c.createdAt }}</span>
                </div>
                <div class="gh-comment-body"><MarkdownView :source="c.body" /></div>
              </div>
            </template>
          </div>
        </template>
        <div v-else class="gh-detail-placeholder">← 选择一个 PR 查看详情</div>
      </section>
    </div>

    <!-- create PR modal -->
    <a-modal v-model:open="createOpen" title="创建 Pull Request" width="600px" :footer="null" destroy-on-close>
      <div style="display:flex;gap:12px;margin-bottom:12px">
        <a-input v-model:value="createForm.title" placeholder="标题" style="flex:1" />
      </div>
      <div style="display:flex;gap:12px;margin-bottom:12px">
        <a-input v-model:value="createForm.head" placeholder="源分支 (head)" style="flex:1" />
        <span style="line-height:30px">→</span>
        <a-input v-model:value="createForm.base" placeholder="目标分支 (base)" style="flex:1" />
      </div>
      <MdEditor v-model="createForm.body" :theme="mdTheme" language="zh-CN"
        :toolbars="['bold','italic','code','codeRow','link']" :preview="true"
        style="height:200px;margin-bottom:12px;border-radius:var(--radius-sm);overflow:hidden;" />
      <div style="display:flex;justify-content:flex-end;gap:8px">
        <a-button size="small" @click="createOpen = false">取消</a-button>
        <a-button size="small" type="primary" :loading="creating" @click="doCreate">创建</a-button>
      </div>
    </a-modal>

    <!-- comment modal -->
    <a-modal v-model:open="commentOpen" title="发表评论" width="500px" :footer="null" destroy-on-close>
      <a-textarea v-model:value="commentBody" placeholder="输入评论…" :auto-size="{ minRows: 3, maxRows: 8 }" />
      <div style="display:flex;justify-content:flex-end;gap:8px;margin-top:12px">
        <a-button size="small" @click="commentOpen = false">取消</a-button>
        <a-button size="small" type="primary" :loading="commenting" @click="doComment">发送</a-button>
      </div>
    </a-modal>

    <!-- review modal -->
    <a-modal v-model:open="reviewOpen" :title="reviewEvent === 'APPROVE' ? '批准 PR' : '请求修改'" width="500px" :footer="null" destroy-on-close>
      <a-textarea v-model:value="reviewBody" placeholder="评论（可选）" :auto-size="{ minRows: 3, maxRows: 8 }" />
      <div style="display:flex;justify-content:flex-end;gap:8px;margin-top:12px">
        <a-button size="small" @click="reviewOpen = false">取消</a-button>
        <a-button size="small" type="primary" :loading="reviewing" @click="doReview">提交</a-button>
      </div>
    </a-modal>
  </div>
</template>

<style scoped>
.gh-view { width: calc(100% + var(--spacing-lg) * 2); height: calc(100% + var(--spacing-lg) * 2); margin: calc(-1 * var(--spacing-lg)); display: flex; flex-direction: column; background: transparent; overflow: hidden; }

.gh-guide { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 10px; }
.gh-guide-icon { font-size: 48px; opacity: 0.4; color: var(--color-text-tertiary); }
.gh-guide-icon.warn { color: var(--color-warning); opacity: 0.7; }
.gh-guide-title { font-size: 16px; font-weight: 600; color: var(--color-text); }

/* two-pane ready layout */
.gh-ready { flex: 1; display: flex; gap: 1px; background: var(--color-border); min-height: 0; overflow: hidden; }

/* left: PR list */
.gh-list-pane { width: 300px; flex-shrink: 0; background: var(--color-surface); display: flex; flex-direction: column; min-height: 0; }
.gh-head { display: flex; align-items: center; gap: 6px; height: 34px; padding: 0 12px; border-bottom: 1px solid var(--color-border); flex-shrink: 0; }
.gh-repo { font-family: 'Cascadia Code', monospace; font-size: 13px; font-weight: 600; color: var(--color-text); flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.gh-head-btns { display: flex; align-items: center; gap: 6px; flex-shrink: 0; }
.gh-filter-bar { display: flex; align-items: center; gap: 6px; padding: 5px 10px; border-bottom: 1px solid var(--color-border-light); flex-shrink: 0; }
.gh-filter-bar-2 { border-bottom: 1px solid var(--color-border); }
.gh-filter-bar-2 :deep(.ant-input) { width: 100%; }
.gh-pr-state-closed { font-size: 9px; padding: 0 4px; border-radius: 3px; background: var(--color-error-bg); color: var(--color-error); font-weight: 600; flex-shrink: 0; }
.gh-pr-list { flex: 1; overflow-y: auto; padding: 4px; }
.gh-center { display: flex; align-items: center; justify-content: center; height: 200px; }
.gh-empty { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 6px; color: var(--color-text-tertiary); font-size: 13px; height: 200px; }

.gh-pr-card { padding: 8px 10px; border-radius: var(--radius-xs); cursor: pointer; border-left: 2px solid transparent; transition: all var(--transition-fast); }
.gh-pr-card:hover { background: var(--color-hover); }
.gh-pr-card.active { background: var(--color-primary-bg); border-left-color: var(--color-primary); }
.gh-pr-title-row { display: flex; align-items: baseline; gap: 6px; }
.gh-pr-num { font-family: 'Cascadia Code', monospace; font-size: 11px; color: var(--color-text-tertiary); flex-shrink: 0; }
.gh-pr-title { font-size: 13px; color: var(--color-text); font-weight: 500; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.gh-pr-card.active .gh-pr-title { color: var(--color-primary); }
.gh-pr-meta { display: flex; align-items: center; gap: 8px; margin-top: 3px; font-size: 10px; color: var(--color-text-tertiary); }
.gh-pr-branch { display: flex; align-items: center; gap: 2px; font-family: 'Cascadia Code', monospace; }
.gh-pr-author { color: var(--color-text-secondary); }
.gh-pr-draft { font-size: 9px; padding: 0 4px; border-radius: 3px; background: var(--color-warning-bg); color: var(--color-warning); }

/* right: PR detail */
.gh-detail-pane { flex: 1; min-width: 0; background: var(--color-surface); display: flex; flex-direction: column; min-height: 0; }
.gh-detail-placeholder { flex: 1; display: flex; align-items: center; justify-content: center; color: var(--color-text-tertiary); font-size: 14px; }
.gh-detail-head { display: flex; align-items: center; gap: 8px; height: 34px; padding: 0 14px; border-bottom: 1px solid var(--color-border); flex-shrink: 0; }
.gh-detail-title { font-size: 14px; font-weight: 600; color: var(--color-text); flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.gh-detail-actions { display: flex; gap: 2px; flex-shrink: 0; }
.gh-detail-body-preview { padding: 8px 14px; border-bottom: 1px solid var(--color-border-light); font-size: 13px; max-height: 120px; overflow-y: auto; flex-shrink: 0; }

.gh-detail-tabs { display: flex; gap: 2px; padding: 4px 12px; border-bottom: 1px solid var(--color-border-light); flex-shrink: 0; }
.gh-tab {
  -webkit-appearance: none; appearance: none; border: none; background: transparent; cursor: pointer;
  font-size: 12px; color: var(--color-text-tertiary); padding: 4px 10px; border-radius: var(--radius-xs);
  display: inline-flex; align-items: center; gap: 4px; transition: all var(--transition-fast);
}
.gh-tab:hover { background: var(--color-hover); }
.gh-tab.active { background: var(--color-primary-bg); color: var(--color-primary); font-weight: 500; }

.gh-detail-body { flex: 1; overflow-y: auto; padding: 8px 14px; min-height: 0; }
.gh-detail-spin { display: flex; justify-content: center; padding: 40px; }
.gh-detail-empty { padding: 30px; text-align: center; color: var(--color-text-tertiary); font-size: 13px; }

/* file item */
.gh-file-item { margin-bottom: 10px; }
.gh-file-head { display: flex; align-items: center; gap: 8px; margin-bottom: 3px; }
.gh-file-status { width: 16px; height: 16px; border-radius: 3px; font-size: 9px; font-weight: 700; display: inline-flex; align-items: center; justify-content: center; flex-shrink: 0; }
.fs-added, .fs-A { color: var(--color-success); background: var(--color-success-bg); }
.fs-modified, .fs-M { color: var(--color-warning); background: var(--color-warning-bg); }
.fs-removed, .fs-D { color: var(--color-error); background: var(--color-error-bg); }
.fs-renamed, .fs-R { color: var(--color-info); background: var(--color-info-bg); }
.gh-file-name { flex: 1; font-family: 'Cascadia Code', monospace; font-size: 12px; color: var(--color-text); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.gh-file-delta { font-family: 'Cascadia Code', monospace; font-size: 11px; flex-shrink: 0; }
.gh-file-patch {
  font-family: 'Cascadia Code', monospace; font-size: 11px; line-height: 1.5;
  background: var(--color-background); padding: 8px 10px; border-radius: var(--radius-xs);
  white-space: pre-wrap; word-break: break-word; margin: 0; max-height: 300px; overflow-y: auto;
}

/* comment item */
.gh-comment-item { padding: 8px 10px; border-radius: var(--radius-xs); background: var(--color-background); margin-bottom: 6px; }
.gh-comment-head { display: flex; align-items: center; gap: 8px; margin-bottom: 4px; font-size: 11px; }
.gh-comment-author { color: var(--color-text-secondary); font-weight: 500; }
.gh-comment-path { font-family: 'Cascadia Code', monospace; color: var(--color-primary); font-size: 10px; }
.gh-comment-date { color: var(--color-text-tertiary); margin-left: auto; }
.gh-comment-body { font-size: 12px; color: var(--color-text); white-space: pre-wrap; word-break: break-word; }
</style>
