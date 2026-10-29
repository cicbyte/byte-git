<script setup>
import { ref, computed, onMounted, watch, onBeforeUnmount, h } from 'vue'
import { useRouter } from 'vue-router'
import { message as AMessage, Modal } from 'ant-design-vue'
import {
  GithubOutlined, ReloadOutlined, PlayCircleOutlined, StopOutlined,
  ExclamationCircleOutlined, CheckCircleOutlined, LoadingOutlined,
  CloseCircleOutlined, ClockCircleOutlined, FileTextOutlined,
} from '@ant-design/icons-vue'
import { useRepoStore } from '@/stores/repo'
import { usePageFooter } from '@/composables/usePageFooter'
import { useGitHubCache } from '@/composables/useGitHub'

/**
 * GitHub Actions/CI — 工作流运行列表 + rerun/cancel。
 */
const app = window.go?.main?.ByteGitApp
const repo = useRepoStore()
const router = useRouter()
const { status, ensureStatusLoaded } = useGitHubCache()
const { setFooter, clearFooter } = usePageFooter()

const runs = ref([])
const loading = ref(false)
const acting = ref(false)

// detail panel
const selectedRun = ref(null)
const jobs = ref([])
const jobsLoading = ref(false)
const activeJob = ref(null)
const jobLog = ref('')
const logLoading = ref(false)

const canUse = computed(() => status.value.authed && status.value.remoteIsGh)

async function loadRuns() {
  if (!canUse.value || !app?.GhActionsList) return
  loading.value = true
  try {
    runs.value = (await app.GhActionsList()) || []
  } catch (e) {
    AMessage.error('加载 CI 运行失败: ' + String(e?.message || e))
    runs.value = []
  } finally {
    loading.value = false
  }
}

async function selectRun(run) {
  selectedRun.value = run
  jobs.value = []
  activeJob.value = null
  jobLog.value = ''
  if (!run) return
  jobsLoading.value = true
  try {
    jobs.value = (await app.GhActionsJobs(run.id)) || []
  } catch { jobs.value = [] }
  finally { jobsLoading.value = false }
}

async function viewJobLog(job) {
  activeJob.value = job
  jobLog.value = ''
  logLoading.value = true
  try {
    const text = await app.GhActionsJobLogs(job.id)
    if (text) {
      jobLog.value = text
    } else {
      AMessage.warning('无可用日志')
    }
  } catch (e) {
    AMessage.error('获取日志失败: ' + String(e?.message || e))
  } finally {
    logLoading.value = false
  }
}

function updateFooter() {
  if (!canUse.value) return
  const ok = runs.value.filter((r) => r.conclusion === 'success').length
  const fail = runs.value.filter((r) => r.conclusion === 'failure').length
  setFooter(ActionsFooter, () => ({
    user: status.value.user,
    repo: status.value.ownerRepo,
    ok, fail,
    total: runs.value.length,
  }))
}

const ActionsFooter = {
  props: ['user', 'repo', 'ok', 'fail', 'total'],
  render() {
    return h('div', { style: 'display:flex;align-items:center;gap:8px;width:100%;font-size:12px;color:var(--color-text-tertiary)' }, [
      h(GithubOutlined, { style: 'font-size:13px' }),
      h('span', { style: 'font-weight:500;color:var(--color-text)' }, 'Actions'),
      h('span', `${this.repo}`),
      h('div', { style: 'flex:1' }),
      h('span', { style: `color: var(--color-success)` }, `✓ ${this.ok}`),
      h('span', { style: `color: var(--color-error)` }, `✗ ${this.fail}`),
      h('span', `${this.total} 次运行`),
    ])
  },
}

// ---- status helpers ----
const statusIcon = {
  completed: { success: CheckCircleOutlined, failure: CloseCircleOutlined, cancelled: StopOutlined, null: ClockCircleOutlined },
  in_progress: { null: LoadingOutlined },
  queued: { null: ClockCircleOutlined },
}

function runIcon(run) {
  if (run.status === 'in_progress') return LoadingOutlined
  if (run.status === 'queued') return ClockCircleOutlined
  if (run.conclusion === 'success') return CheckCircleOutlined
  if (run.conclusion === 'failure') return CloseCircleOutlined
  return ClockCircleOutlined
}

function runColor(run) {
  if (run.status === 'in_progress') return 'var(--color-primary)'
  if (run.status === 'queued') return 'var(--color-text-tertiary)'
  if (run.conclusion === 'success') return 'var(--color-success)'
  if (run.conclusion === 'failure') return 'var(--color-error)'
  return 'var(--color-text-tertiary)'
}

function runLabel(run) {
  if (run.status === 'in_progress') return '运行中'
  if (run.status === 'queued') return '排队'
  const c = run.conclusion
  if (c === 'success') return '成功'
  if (c === 'failure') return '失败'
  if (c === 'cancelled') return '已取消'
  return c || '未知'
}

// ---- actions ----
function confirmRerun(run) {
  Modal.confirm({
    title: `重新运行？`,
    content: `${run.name || run.commit} — 将触发新的 CI 运行`,
    okText: '重新运行', cancelText: '取消',
    async onOk() {
      acting.value = true
      try { await app.GhActionsRerun(run.id); AMessage.success('已触发重新运行'); await loadRuns() }
      catch (e) { AMessage.error('重新运行失败: ' + String(e?.message || e)) }
      finally { acting.value = false }
    },
  })
}

function confirmCancel(run) {
  Modal.confirm({
    title: `取消运行？`,
    content: `${run.name || run.commit} — 将终止当前 CI 运行`,
    okText: '取消运行', okType: 'danger', cancelText: '返回',
    async onOk() {
      acting.value = true
      try { await app.GhActionsCancel(run.id); AMessage.success('已取消'); await loadRuns() }
      catch (e) { AMessage.error('取消失败: ' + String(e?.message || e)) }
      finally { acting.value = false }
    },
  })
}

onMounted(async () => {
  await ensureStatusLoaded()
  if (canUse.value) { await loadRuns(); updateFooter() }
})
onBeforeUnmount(() => clearFooter())

watch(() => repo.repoEpoch, () => { loadRuns() })
</script>

<template>
  <div class="ga-view">
    <!-- not ready -->
    <div v-if="!status.tokenSet" class="ga-guide">
      <GithubOutlined class="ga-guide-icon" />
      <div class="ga-guide-title">需要配置 GitHub Token</div>
      <a-button type="primary" size="small" @click="router.push('/settings')">前往设置</a-button>
    </div>
    <div v-else-if="!status.authed" class="ga-guide">
      <ExclamationCircleOutlined class="ga-guide-icon warn" />
      <div class="ga-guide-title">Token 无效</div>
      <a-button type="primary" size="small" @click="router.push('/settings')">前往设置</a-button>
    </div>
    <div v-else-if="!status.remoteIsGh" class="ga-guide">
      <ExclamationCircleOutlined class="ga-guide-icon warn" />
      <div class="ga-guide-title">当前仓库不是 GitHub 仓库</div>
    </div>

    <!-- ready: two-pane layout -->
    <div v-else class="ga-ready">
      <!-- left: run list -->
      <aside class="ga-list-pane">
        <div class="ga-head">
          <span class="ga-repo">{{ status.ownerRepo }}</span>
          <a-button size="small" type="text" :loading="loading" @click="loadRuns" title="刷新">
            <template #icon><ReloadOutlined /></template>
          </a-button>
        </div>
        <div class="ga-run-list">
          <div v-if="loading && !runs.length" class="ga-center"><a-spin /></div>
          <div v-else-if="!runs.length" class="ga-empty">
            <CheckCircleOutlined style="font-size:28px;opacity:0.4" />
            <div style="margin-top:6px">没有 CI 运行记录</div>
          </div>
          <div v-for="run in runs" :key="run.id" class="ga-card"
            :class="{ active: selectedRun?.id === run.id }"
            :style="{ borderLeftColor: runColor(run) }"
            @click="selectRun(run)"
          >
            <component :is="runIcon(run)" class="ga-run-icon" :style="{ color: runColor(run) }" :spin="run.status === 'in_progress'" />
            <div class="ga-main">
              <div class="ga-title-row">
                <span class="ga-name">{{ run.name || 'CI' }}</span>
                <span class="ga-badge" :style="{ color: runColor(run), background: runColor(run) + '15' }">{{ runLabel(run) }}</span>
              </div>
              <div class="ga-meta">
                <span class="ga-branch">{{ run.branch }}</span>
                <span class="ga-commit">{{ run.commit }}</span>
                <span class="ga-event">{{ run.event }}</span>
                <span class="ga-date">{{ run.createdAt }}</span>
              </div>
            </div>
          </div>
        </div>
      </aside>

      <!-- right: detail (jobs) -->
      <section class="ga-detail-pane">
        <template v-if="selectedRun">
          <div class="ga-detail-head">
            <span class="ga-detail-title">{{ selectedRun.name || 'CI' }} · {{ selectedRun.branch }} ({{ selectedRun.commit }})</span>
            <div class="ga-detail-actions">
              <a-button v-if="selectedRun.status === 'in_progress'" size="small" type="text" danger :loading="acting" @click="confirmCancel(selectedRun)" title="取消"><StopOutlined /></a-button>
              <a-button v-if="selectedRun.status === 'completed'" size="small" type="text" :loading="acting" @click="confirmRerun(selectedRun)" title="重新运行"><PlayCircleOutlined /></a-button>
              <a-button size="small" type="text" :href="selectedRun.htmlUrl" target="_blank" title="浏览器打开"><GithubOutlined /></a-button>
            </div>
          </div>
          <div class="ga-detail-body">
            <div v-if="jobsLoading" class="ga-center"><a-spin /></div>
            <div v-else-if="!jobs.length" class="ga-detail-empty">无 Job 信息</div>
            <div v-for="job in jobs" :key="job.id" class="ga-job-card" :style="{ borderLeftColor: runColor(job) }">
              <component :is="runIcon(job)" class="ga-job-icon" :style="{ color: runColor(job) }" :spin="job.status === 'in_progress'" />
              <div class="ga-job-main">
                <div class="ga-job-name">{{ job.name }}</div>
                <div class="ga-job-meta">
                  <span class="ga-job-badge" :style="{ color: runColor(job), background: runColor(job) + '15' }">{{ runLabel(job) }}</span>
                  <span v-if="job.startedAt" class="ga-job-date">{{ job.startedAt }}</span>
                </div>
              </div>
              <div class="ga-job-actions">
                <a-button size="small" type="text" :loading="logLoading && activeJob?.id === job.id" @click="viewJobLog(job)" title="查看日志">
                  <template #icon><FileTextOutlined /></template>
                </a-button>
                <a-button size="small" type="text" :href="job.htmlUrl" target="_blank" title="浏览器打开">
                  <template #icon><GithubOutlined /></template>
                </a-button>
              </div>
            </div>
            <!-- inline log viewer -->
            <div v-if="activeJob?.id === job.id && (jobLog || logLoading)" class="ga-log-panel">
              <div class="ga-log-head">
                <span class="ga-log-title">日志: {{ job.name }}</span>
                <a-button size="small" type="text" @click="activeJob = null" title="关闭">
                  <template #icon><CloseCircleOutlined /></template>
                </a-button>
              </div>
              <a-spin v-if="logLoading" size="small" class="ga-log-spin" />
              <pre v-else class="ga-log-text">{{ jobLog }}</pre>
            </div>
          </div>
        </template>
        <div v-else class="ga-detail-placeholder">← 选择一个运行查看 Jobs</div>
      </section>
    </div>
  </div>
</template>

<style scoped>
.ga-view { width: calc(100% + var(--spacing-lg) * 2); height: calc(100% + var(--spacing-lg) * 2); margin: calc(-1 * var(--spacing-lg)); display: flex; flex-direction: column; background: transparent; overflow: hidden; }

.ga-guide { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 10px; }
.ga-guide-icon { font-size: 48px; color: var(--color-text-tertiary); opacity: 0.4; }
.ga-guide-icon.warn { color: var(--color-warning); opacity: 0.7; }
.ga-guide-title { font-size: 16px; font-weight: 600; color: var(--color-text); }

.ga-ready { flex: 1; display: flex; gap: 1px; background: var(--color-border); min-height: 0; overflow: hidden; }

/* left: run list */
.ga-list-pane { width: 340px; flex-shrink: 0; background: var(--color-surface); display: flex; flex-direction: column; min-height: 0; }
.ga-head { display: flex; align-items: center; gap: 6px; height: 34px; padding: 0 12px; border-bottom: 1px solid var(--color-border); flex-shrink: 0; }
.ga-repo { font-family: 'Cascadia Code', monospace; font-size: 13px; font-weight: 600; color: var(--color-text); margin-right: auto; }

.ga-run-list { flex: 1; overflow-y: auto; padding: 4px; }
.ga-center { display: flex; align-items: center; justify-content: center; height: 200px; }
.ga-empty { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 4px; color: var(--color-text-tertiary); font-size: 13px; height: 200px; }

.ga-card {
  display: flex; align-items: center; gap: 10px;
  padding: 8px 10px; border-radius: var(--radius-xs);
  border: 1px solid transparent; border-left: 3px solid transparent;
  cursor: pointer; transition: all var(--transition-fast);
}
.ga-card:hover { background: var(--color-hover); }
.ga-card.active { background: var(--color-primary-bg); border-left-color: var(--color-primary) !important; }

/* right: detail */
.ga-detail-pane { flex: 1; min-width: 0; background: var(--color-surface); display: flex; flex-direction: column; min-height: 0; }
.ga-detail-placeholder { flex: 1; display: flex; align-items: center; justify-content: center; color: var(--color-text-tertiary); font-size: 14px; }
.ga-detail-head { display: flex; align-items: center; gap: 8px; height: 34px; padding: 0 14px; border-bottom: 1px solid var(--color-border); flex-shrink: 0; }
.ga-detail-title { font-size: 13px; font-weight: 600; color: var(--color-text); flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.ga-detail-actions { display: flex; gap: 2px; flex-shrink: 0; }
.ga-detail-body { flex: 1; overflow-y: auto; padding: 8px 14px; min-height: 0; }
.ga-detail-empty { padding: 30px; text-align: center; color: var(--color-text-tertiary); font-size: 13px; }

/* job card */
.ga-job-card {
  display: flex; align-items: center; gap: 8px;
  padding: 8px 12px; border-radius: var(--radius-xs);
  background: var(--color-background); border-left: 3px solid transparent;
  margin-bottom: 4px;
}
.ga-job-icon { font-size: 16px; flex-shrink: 0; }
.ga-job-main { flex: 1; min-width: 0; }
.ga-job-name { font-size: 13px; color: var(--color-text); font-weight: 500; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.ga-job-meta { display: flex; align-items: center; gap: 6px; margin-top: 2px; font-size: 11px; color: var(--color-text-tertiary); }
.ga-job-badge { font-size: 10px; font-weight: 600; padding: 1px 5px; border-radius: 3px; }
.ga-job-date { font-family: 'Cascadia Code', monospace; }
.ga-job-actions { display: flex; gap: 2px; flex-shrink: 0; }

/* inline log viewer */
.ga-log-panel {
  margin-top: 4px; border-radius: var(--radius-xs);
  background: #0f172a; overflow: hidden;
}
.ga-log-head {
  display: flex; align-items: center; justify-content: space-between;
  padding: 4px 10px; background: rgba(255,255,255,0.05);
}
.ga-log-title { font-size: 11px; color: #94a3b8; font-weight: 500; }
.ga-log-spin { display: flex; justify-content: center; padding: 20px; }
.ga-log-text {
  font-family: 'Cascadia Code', 'JetBrains Mono', monospace; font-size: 11px;
  line-height: 1.5; color: #e2e8f0; margin: 0; padding: 8px 10px;
  white-space: pre-wrap; word-break: break-word;
  max-height: 400px; overflow-y: auto;
}
.ga-log-text::-webkit-scrollbar { width: 5px; }
.ga-log-text::-webkit-scrollbar-thumb { background: rgba(148,163,184,0.3); border-radius: 3px; }

.ga-run-icon { font-size: 18px; flex-shrink: 0; }

.ga-main { flex: 1; min-width: 0; }
.ga-title-row { display: flex; align-items: center; gap: 8px; }
.ga-name { font-size: 14px; color: var(--color-text); font-weight: 500; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.ga-badge { font-size: 10px; font-weight: 600; padding: 1px 6px; border-radius: 3px; flex-shrink: 0; }
.ga-meta { display: flex; align-items: center; gap: 10px; margin-top: 4px; font-size: 11px; color: var(--color-text-tertiary); flex-wrap: wrap; }
.ga-branch { font-family: 'Cascadia Code', monospace; color: var(--color-text-secondary); }
.ga-commit { font-family: 'Cascadia Code', monospace; }
.ga-event { color: var(--color-primary); }
.ga-actor { color: var(--color-text-secondary); }
.ga-date { font-family: 'Cascadia Code', monospace; }

.ga-actions { display: flex; gap: 2px; flex-shrink: 0; }
</style>
