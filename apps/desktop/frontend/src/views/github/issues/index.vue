<script setup>
import { ref, computed, onMounted, watch, onBeforeUnmount, h } from 'vue'
import { useRouter } from 'vue-router'
import { message as AMessage, Modal } from 'ant-design-vue'
import {
  GithubOutlined, ReloadOutlined, PlusOutlined, CloseOutlined,
  CommentOutlined, ExclamationCircleOutlined, CheckCircleOutlined,
  TagOutlined,
} from '@ant-design/icons-vue'
import { useRepoStore } from '../../../stores/repo'
import { usePageFooter } from '../../../composables/usePageFooter'
import { useGitHubCache } from '../../../composables/useGitHub'
import { MdEditor } from 'md-editor-v3'
import 'md-editor-v3/lib/style.css'
import { useThemeStore } from '../../../stores/theme'

/**
 * GitHub Issues 管理 — 列表 + 创建 + 关闭 + 评论。
 * 认证状态复用 useGitHubCache，不重复检测。
 */
const app = window.go?.main?.ByteGitApp
const repo = useRepoStore()
const router = useRouter()
const { status, ensureStatusLoaded, invalidatePRs } = useGitHubCache()
const { setFooter, clearFooter } = usePageFooter()
const themeStore = useThemeStore()
const mdTheme = computed(() => (themeStore.isDark ? 'dark' : 'light'))

const issues = ref([])
const loading = ref(false)
const acting = ref(false)
const activeIssue = ref(null)
const repoLabels = ref([])
const issueState = ref('open')
const searchQuery = ref('')

const filteredIssues = computed(() => {
  const q = searchQuery.value.trim().toLowerCase()
  if (!q) return issues.value
  return issues.value.filter((i) =>
    i.title.toLowerCase().includes(q) ||
    i.author.toLowerCase().includes(q) ||
    String(i.number).includes(q),
  )
})

// create modal
const createOpen = ref(false)
const createTitle = ref('')
const createBody = ref('')
const creating = ref(false)

// comment modal
const commentOpen = ref(false)
const commentBody = ref('')
const commenting = ref(false)
const commentTarget = ref(null)

const canUse = computed(() => status.value.authed && status.value.remoteIsGh)

watch(issues, (v) => { if (v.length && !activeIssue.value) activeIssue.value = v[0] }, { immediate: true })

// label modal
const labelOpen = ref(false)
const labelTarget = ref(null)
const labelEditing = ref(false)

async function loadLabels() {
  if (!app?.GhIssueLabels) return
  try { repoLabels.value = (await app.GhIssueLabels()) || [] } catch { repoLabels.value = [] }
}

async function openLabels(issue) {
  labelTarget.value = issue
  labelOpen.value = true
  if (!repoLabels.value.length) await loadLabels()
}

function isLabelOn(label) {
  return labelTarget.value?.labels?.includes(label)
}

async function toggleLabel(label) {
  if (!labelTarget.value) return
  labelEditing.value = true
  try {
    if (isLabelOn(label)) {
      await app.GhIssueRemoveLabel(labelTarget.value.number, label)
      labelTarget.value.labels = labelTarget.value.labels.filter((l) => l !== label)
    } else {
      await app.GhIssueAddLabels(labelTarget.value.number, [label])
      if (!labelTarget.value.labels) labelTarget.value.labels = []
      labelTarget.value.labels.push(label)
    }
    // update list
    const idx = issues.value.findIndex((i) => i.number === labelTarget.value.number)
    if (idx >= 0) issues.value[idx].labels = labelTarget.value.labels
  } catch (e) { AMessage.error('标签操作失败: ' + String(e?.message || e)) }
  finally { labelEditing.value = false }
}

async function loadIssues() {
  if (!canUse.value || !app?.GhIssueList) return
  loading.value = true
  try {
    issues.value = (await app.GhIssueList(issueState.value)) || []
  } catch (e) {
    AMessage.error('加载 Issue 失败: ' + String(e?.message || e))
    issues.value = []
  } finally {
    loading.value = false
  }
}

function updateFooter() {
  if (!canUse.value) return
  setFooter(IssueFooter, () => ({
    user: status.value.user,
    repo: status.value.ownerRepo,
    count: issues.value.length,
  }))
}

const IssueFooter = {
  props: ['user', 'repo', 'count'],
  render() {
    return h('div', { style: 'display:flex;align-items:center;gap:8px;width:100%;font-size:12px;color:var(--color-text-tertiary)' }, [
      h(GithubOutlined, { style: 'font-size:13px' }),
      h('span', { style: 'font-weight:500;color:var(--color-text)' }, 'Issues'),
      h('span', `${this.user} · ${this.repo}`),
      h('div', { style: 'flex:1' }),
      h('span', `${this.count} 个 Issue`),
    ])
  },
}

// ---- actions ----
function openCreate() {
  createTitle.value = ''
  createBody.value = ''
  createOpen.value = true
}

async function doCreate() {
  if (!createTitle.value.trim()) { AMessage.warning('请输入标题'); return }
  creating.value = true
  try {
    await app.GhIssueCreate(createTitle.value.trim(), createBody.value)
    AMessage.success('Issue 已创建')
    createOpen.value = false
    await loadIssues()
  } catch (e) {
    AMessage.error('创建失败: ' + String(e?.message || e))
  } finally {
    creating.value = false
  }
}

function confirmClose(issue) {
  Modal.confirm({
    title: `关闭 Issue #${issue.number}?`,
    content: issue.title,
    okText: '关闭', okType: 'danger', cancelText: '取消',
    async onOk() {
      acting.value = true
      try {
        await app.GhIssueClose(issue.number)
        AMessage.success(`已关闭 Issue #${issue.number}`)
        await loadIssues()
      } catch (e) { AMessage.error('关闭失败: ' + String(e?.message || e)) }
      finally { acting.value = false }
    },
  })
}

function openComment(issue) {
  commentTarget.value = issue
  commentBody.value = ''
  commentOpen.value = true
}

async function doComment() {
  if (!commentBody.value.trim()) { AMessage.warning('请输入评论内容'); return }
  commenting.value = true
  try {
    await app.GhIssueComment(commentTarget.value.number, commentBody.value)
    AMessage.success('评论已发送')
    commentOpen.value = false
  } catch (e) {
    AMessage.error('评论失败: ' + String(e?.message || e))
  } finally {
    commenting.value = false
  }
}

onMounted(async () => {
  await ensureStatusLoaded()
  if (canUse.value) {
    await loadIssues()
    loadLabels()
    updateFooter()
  }
})
onBeforeUnmount(() => clearFooter())

watch(() => repo.repoEpoch, () => { invalidatePRs(); loadIssues() })
</script>

<template>
  <div class="gi-view">
    <!-- not ready -->
    <div v-if="!status.tokenSet" class="gi-guide">
      <GithubOutlined class="gi-guide-icon" />
      <div class="gi-guide-title">需要配置 GitHub Token</div>
      <a-button type="primary" size="small" @click="router.push('/settings')">前往设置</a-button>
    </div>
    <div v-else-if="!status.authed" class="gi-guide">
      <ExclamationCircleOutlined class="gi-guide-icon warn" />
      <div class="gi-guide-title">Token 无效</div>
      <a-button type="primary" size="small" @click="router.push('/settings')">前往设置</a-button>
    </div>
    <div v-else-if="!status.remoteIsGh" class="gi-guide">
      <ExclamationCircleOutlined class="gi-guide-icon warn" />
      <div class="gi-guide-title">当前仓库不是 GitHub 仓库</div>
    </div>

    <!-- ready -->
    <div v-else class="gi-ready">
      <div class="gi-head">
        <span class="gi-repo">{{ status.ownerRepo }}</span>
        <a-button size="small" @click="openCreate">
          <template #icon><PlusOutlined /></template>新建
        </a-button>
      </div>
      <div class="gi-filter-bar">
        <a-radio-group v-model:value="issueState" size="small" @change="() => loadIssues()">
          <a-radio-button value="open">Open</a-radio-button>
          <a-radio-button value="closed">Closed</a-radio-button>
          <a-radio-button value="all">All</a-radio-button>
        </a-radio-group>
        <a-button size="small" type="text" :loading="loading" @click="loadIssues" title="刷新">
          <template #icon><ReloadOutlined /></template>
        </a-button>
      </div>
      <div class="gi-filter-bar gi-filter-bar-2">
        <a-input v-model:value="searchQuery" placeholder="搜索标题/作者/编号…" size="small" allow-clear />
      </div>

      <div class="gi-list">
        <div v-if="loading" class="gi-center"><a-spin /></div>
        <div v-else-if="!filteredIssues.length" class="gi-empty">
          <CheckCircleOutlined style="font-size:32px;color:var(--color-success);opacity:0.5" />
          <div style="margin-top:8px">{{ searchQuery ? '无匹配结果' : '没有 Issue' }}</div>
        </div>
        <div
          v-for="is in filteredIssues" :key="is.number"
          class="gi-card"
          :class="{ active: activeIssue?.number === is.number }"
          @click="activeIssue = is"
        >
          <div class="gi-main">
            <div class="gi-title-row">
              <span class="gi-num">#{{ is.number }}</span>
              <span class="gi-title">{{ is.title }}</span>
            </div>
            <div class="gi-meta">
              <span class="gi-author">@{{ is.author }}</span>
              <span class="gi-date">{{ is.createdAt }}</span>
              <span v-if="is.comments" class="gi-comments">
                <CommentOutlined /> {{ is.comments }}
              </span>
              <span v-for="l in is.labels" :key="l" class="gi-label">{{ l }}</span>
            </div>
          </div>
          <div class="gi-actions">
            <a-button size="small" type="text" @click.stop="openLabels(is)" title="标签">
              <TagOutlined />
            </a-button>
            <a-button size="small" type="text" @click.stop="openComment(is)" title="评论">
              <CommentOutlined />
            </a-button>
            <a-button size="small" type="text" @click.stop="confirmClose(is)" title="关闭">
              <CloseOutlined />
            </a-button>
          </div>
        </div>
      </div>
    </div>

    <!-- create modal -->
    <a-modal v-model:open="createOpen" title="新建 Issue" width="680px" :footer="null" destroy-on-close>
      <div class="gi-create">
        <a-input v-model:value="createTitle" placeholder="标题" style="margin-bottom:12px" />
        <div class="gi-md-label">描述（Markdown）</div>
        <MdEditor
          v-model="createBody"
          :theme="mdTheme"
          language="zh-CN"
          :toolbars="['bold','italic','strike','quote','list','orderedList','task','code','codeRow','link','table']"
          :preview="true"
          style="height: 280px; margin-bottom: 12px; border-radius: var(--radius-sm); overflow: hidden;"
        />
        <div style="display:flex;justify-content:flex-end;gap:8px">
          <a-button size="small" @click="createOpen = false">取消</a-button>
          <a-button size="small" type="primary" :loading="creating" @click="doCreate">创建</a-button>
        </div>
      </div>
    </a-modal>

    <!-- comment modal -->
    <a-modal v-model:open="commentOpen" title="发表评论" width="600px" :footer="null" destroy-on-close>
      <div v-if="commentTarget" class="gi-comment-target">
        #{{ commentTarget.number }} {{ commentTarget.title }}
      </div>
      <MdEditor
        v-model="commentBody"
        :theme="mdTheme"
        language="zh-CN"
        :toolbars="['bold','italic','strike','quote','code','codeRow','link']"
        :preview="true"
        style="height: 200px; margin-top: 8px; border-radius: var(--radius-sm); overflow: hidden;"
      />
      <div style="display:flex;justify-content:flex-end;gap:8px;margin-top:12px">
        <a-button size="small" @click="commentOpen = false">取消</a-button>
        <a-button size="small" type="primary" :loading="commenting" @click="doComment">发送</a-button>
      </div>
    </a-modal>

    <!-- label modal -->
    <a-modal v-model:open="labelOpen" title="管理标签" width="440px" :footer="null" destroy-on-close>
      <div v-if="labelTarget" class="gi-label-target">
        #{{ labelTarget.number }} {{ labelTarget.title }}
      </div>
      <div class="gi-label-list">
        <div v-if="!repoLabels.length" style="color:var(--color-text-tertiary);font-size:12px;padding:12px 0">无可用标签</div>
        <button
          v-for="l in repoLabels" :key="l.name"
          class="gi-label-chip"
          :class="{ on: isLabelOn(l.name) }"
          :disabled="labelEditing"
          @click="toggleLabel(l.name)"
        >
          <span class="gi-label-dot" :style="{ background: '#' + l.color }"></span>
          {{ l.name }}
        </button>
      </div>
    </a-modal>
  </div>
</template>

<style scoped>
.gi-view { width: 100%; height: 100%; display: flex; flex-direction: column; background: var(--color-background); padding: var(--spacing-md); overflow: hidden; }

.gi-guide { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 10px; }
.gi-guide-icon { font-size: 48px; color: var(--color-text-tertiary); opacity: 0.4; }
.gi-guide-icon.warn { color: var(--color-warning); opacity: 0.7; }
.gi-guide-title { font-size: 16px; font-weight: 600; color: var(--color-text); }

.gi-ready { flex: 1; display: flex; flex-direction: column; min-height: 0; }
.gi-head { display: flex; align-items: center; gap: 8px; margin-bottom: var(--spacing-xs); flex-shrink: 0; }
.gi-repo { font-family: 'Cascadia Code', monospace; font-size: 14px; font-weight: 600; color: var(--color-text); margin-right: auto; }
.gi-filter-bar { display: flex; align-items: center; gap: 6px; margin-bottom: var(--spacing-xs); flex-shrink: 0; }
.gi-filter-bar-2 { margin-bottom: var(--spacing-sm); }
.gi-filter-bar-2 :deep(.ant-input) { width: 100%; }

.gi-list { flex: 1; overflow-y: auto; display: flex; flex-direction: column; gap: 6px; }
.gi-center { display: flex; align-items: center; justify-content: center; height: 200px; }
.gi-empty { display: flex; flex-direction: column; align-items: center; justify-content: center; color: var(--color-text-tertiary); font-size: 13px; height: 200px; }

.gi-card {
  display: flex; align-items: center; gap: 8px;
  padding: 10px 14px; border-radius: var(--radius-sm);
  background: var(--color-surface); border: 1px solid var(--color-border);
  cursor: pointer; transition: all var(--transition-fast);
}
.gi-card:hover { border-color: var(--color-primary); }
.gi-card.active { border-left: 3px solid var(--color-primary); }

.gi-main { flex: 1; min-width: 0; }
.gi-title-row { display: flex; align-items: baseline; gap: 8px; }
.gi-num { font-family: 'Cascadia Code', monospace; font-size: 12px; color: var(--color-text-tertiary); flex-shrink: 0; }
.gi-title { font-size: 14px; color: var(--color-text); font-weight: 500; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.gi-meta { display: flex; align-items: center; gap: 10px; margin-top: 4px; font-size: 11px; color: var(--color-text-tertiary); flex-wrap: wrap; }
.gi-author { color: var(--color-text-secondary); }
.gi-date { font-family: 'Cascadia Code', monospace; }
.gi-comments { display: inline-flex; align-items: center; gap: 3px; }
.gi-label {
  font-size: 9px; padding: 1px 6px; border-radius: 3px;
  background: var(--color-info-bg); color: var(--color-info); font-weight: 600;
}

.gi-actions { display: flex; gap: 2px; flex-shrink: 0; }

.gi-create { display: flex; flex-direction: column; }
.gi-md-label { font-size: 12px; font-weight: 600; color: var(--color-text-secondary); margin-bottom: 4px; }
.gi-comment-target { font-size: 13px; color: var(--color-text); font-weight: 500; padding: 8px 12px; background: var(--color-background); border-radius: var(--radius-sm); }
.gi-label-target { font-size: 13px; color: var(--color-text); font-weight: 500; padding: 8px 12px; background: var(--color-background); border-radius: var(--radius-sm); margin-bottom: 12px; }
.gi-label-list { display: flex; flex-wrap: wrap; gap: 6px; }
.gi-label-chip {
  -webkit-appearance: none; appearance: none;
  display: inline-flex; align-items: center; gap: 4px;
  padding: 4px 10px; border-radius: 12px;
  border: 1px solid var(--color-border); background: var(--color-background);
  font-size: 12px; color: var(--color-text-secondary); cursor: pointer;
  transition: all var(--transition-fast);
}
.gi-label-chip:hover { border-color: var(--color-primary); color: var(--color-primary); }
.gi-label-chip.on { background: var(--color-primary-bg); border-color: var(--color-primary); color: var(--color-primary); font-weight: 500; }
.gi-label-chip:disabled { opacity: 0.5; cursor: wait; }
.gi-label-dot { width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0; }
</style>
