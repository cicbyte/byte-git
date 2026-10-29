<script setup>
import { ref, computed, onMounted, watch, nextTick } from 'vue'
import { message as AMessage, Modal } from 'ant-design-vue'
import {
  ScanOutlined, SafetyCertificateOutlined, CheckCircleOutlined,
  ExclamationCircleOutlined, WarningOutlined, KeyOutlined, CodeOutlined,
  ToolOutlined, ArrowRightOutlined, HistoryOutlined
} from '@ant-design/icons-vue'
import { useRepoStore } from '@/stores/repo'
import { useToolStatus } from '@/composables/useToolStatus'
import CodeViewer from '@/components/common/CodeViewer.vue'
import SecretResolvePanel from './components/SecretResolvePanel.vue'

const repo = useRepoStore()
const app = window.go?.main?.ByteGitApp

// ---- gitleaks availability (shared cache with the global footer) ----
const { gitleaks: gitleaksStatus, ensureLoaded, refresh: refreshTools } = useToolStatus()

onMounted(() => {
  ensureLoaded() // populates gitleaksStatus from cache (or triggers a check)
  loadScrubAutoBackupDefault()
})

// ---- scan state ----
const running = ref(false)
const result = ref(null)
const maskSecret = ref(true)

const fileGroups = computed(() => {
  if (!result.value?.findings) return []
  const map = new Map()
  for (const f of result.value.findings) {
    if (!map.has(f.File)) map.set(f.File, [])
    map.get(f.File).push(f)
  }
  return [...map.entries()].map(([file, findings]) => ({ file, findings }))
})

const activeFile = ref('')
const activeFindings = computed(() =>
  activeFile.value ? (fileGroups.value.find((g) => g.file === activeFile.value)?.findings || []) : []
)
const findingCount = computed(() => result.value?.findings?.length || 0)

async function runScan() {
  if (!app?.SecretScan) return
  if (!gitleaksStatus.value.available) {
    AMessage.warning('未检测到 gitleaks，请先安装')
    return
  }
  running.value = true
  result.value = null
  try {
    result.value = await app.SecretScan()
    if (findingCount.value) {
      activeFile.value = fileGroups.value[0]?.file || ''
      AMessage.warning(`发现 ${findingCount.value} 处可能的密钥泄露`)
    } else {
      AMessage.success('未发现密钥泄露')
    }
  } catch (e) {
    AMessage.error('扫描失败: ' + String(e?.message || e))
  } finally {
    running.value = false
  }
}

// ---- source view (CodeMirror) ----
const activeTab = ref('detail')
const codeViewerRef = ref(null)
const fileContent = ref('')
const fileLoading = ref(false)
const fileInfo = ref({ binary: false, notFound: false, lines: 0 })
const leakLines = computed(() => activeFindings.value.map((f) => f.StartLine).filter(Boolean))

async function loadFileContent() {
  if (!activeFile.value || !activeFindings.value.length || !app?.SecretFileContent) {
    fileContent.value = ''
    return
  }
  const first = activeFindings.value[0]
  fileLoading.value = true
  fileInfo.value = { binary: false, notFound: false, lines: 0 }
  fileContent.value = ''
  try {
    const res = await app.SecretFileContent(first.Commit, first.File)
    fileInfo.value = { binary: res.binary, notFound: res.notFound, lines: res.lines, dataUrl: res.dataUrl || '' }
    fileContent.value = res.content
    if (!res.binary && !res.notFound && leakLines.value.length) {
      await nextTick()
      codeViewerRef.value?.scrollToLine(leakLines.value[0])
    }
  } catch (e) {
    AMessage.error('读取源码失败: ' + String(e?.message || e))
  } finally {
    fileLoading.value = false
  }
}
watch(activeFile, loadFileContent)
function selectFile(file) {
  activeFile.value = file
}

// ---- resolution (解决方案) ----
const resolveBusy = ref(-1) // index of finding being resolved, -1 = idle

async function platformHint(ruleID) {
  if (!app?.SecretPlatformHint) return { url: '', hint: '' }
  try {
    return await app.SecretPlatformHint(ruleID)
  } catch {
    return { url: '', hint: '' }
  }
}
// cache hints per rule to avoid repeated calls
const hintCache = ref({})
async function ensureHint(ruleID) {
  if (hintCache.value[ruleID]) return
  hintCache.value[ruleID] = await platformHint(ruleID)
}

// jump to the resolution tab and pre-load the platform hint for the active rule
function goResolve() {
  activeTab.value = 'resolve'
  if (activeFindings.value[0]) ensureHint(activeFindings.value[0].RuleID)
}

// watch tab switches: pre-load hint when entering resolve tab
watch(activeTab, (t) => {
  if (t === 'resolve' && activeFindings.value[0]) {
    ensureHint(activeFindings.value[0].RuleID)
  }
})

async function doRemoveFromWorkingTree(f, i) {
  if (!app?.SecretRemoveFromWorkingTree) return
  resolveBusy.value = i
  try {
    const res = await app.SecretRemoveFromWorkingTree({
      file: f.File, secret: f.Secret, replacement: 'REMOVED_SECRET',
    })
    if (!res.fileExists) {
      AMessage.warning('文件在工作区不存在（可能已删除或仅在历史中）')
    } else if (res.changedCount === 0) {
      AMessage.info('密钥未在当前文件中找到（可能已修改）')
    } else {
      AMessage.success(res.message)
      loadFileContent()
    }
  } catch (e) {
    AMessage.error('移除失败: ' + String(e?.message || e))
  } finally {
    resolveBusy.value = -1
  }
}

// secret scrub: high-risk; default to auto-backup on (override per call)
const scrubAutoBackup = ref(true)
async function loadScrubAutoBackupDefault() {
  try { scrubAutoBackup.value = await app.RewriteAutoBackupDefault() } catch { /* keep default true */ }
}

function doScrubHistory(f, i) {
  Modal.confirm({
    title: '⚠️ 确认从历史抹除密钥？',
    content: '此操作不可逆，将改写所有受影响 commit 的 hash、移除 origin remote（需重新 git remote add）、已推送的仓库需 force push 且协作者须重新 clone。建议在「备份管理」中提前做整库克隆备份。',
    okText: '确认抹除',
    okType: 'danger',
    cancelText: '取消',
    async onOk() {
      resolveBusy.value = i
      try {
        const res = await app.SecretScrubHistory(f.Secret, scrubAutoBackup.value)
        if (res.ok) {
          AMessage.success(res.message)
          if (res.remoteRemoved) {
            Modal.info({
              title: 'remote 已移除',
              content: 'git-filter-repo 出于安全已移除 origin remote。请重新执行: git remote add origin <url>，然后 git push --force --all',
            })
          }
        } else {
          AMessage.error(res.message || '抹除失败')
        }
      } catch (e) {
        AMessage.error('抹除失败: ' + String(e?.message || e))
      } finally {
        resolveBusy.value = -1
      }
    },
  })
}

// ---- result stats ----
const uniqueRuleCount = computed(() => {
  const s = new Set()
  for (const f of result.value?.findings || []) s.add(f.RuleID)
  return s.size
})
const highEntropyCount = computed(() =>
  (result.value?.findings || []).filter((f) => f.Entropy >= 4).length
)
const ruleStats = computed(() => {
  const m = new Map()
  for (const f of result.value?.findings || []) m.set(f.RuleID, (m.get(f.RuleID) || 0) + 1)
  return [...m.entries()].map(([rule, count]) => ({ rule, count })).sort((a, b) => b.count - a.count)
})
// per-file-group helpers (rules preview + max severity for the list item)
function groupRules(g) {
  const seen = new Set()
  const arr = []
  for (const f of g.findings) {
    if (!seen.has(f.RuleID)) { seen.add(f.RuleID); arr.push(f.RuleID) }
  }
  return arr.slice(0, 3)
}
function groupSeverity(g) {
  let max = 0
  for (const f of g.findings) if (f.Entropy > max) max = f.Entropy
  return max
}
function groupSeverityClass(g) {
  const e = groupSeverity(g)
  if (e >= 4) return 'high'
  if (e >= 3) return 'mid'
  return 'low'
}

// ---- display helpers ----
function locStr(f) {
  if (!f.StartLine) return f.File || ''
  if (f.EndLine > f.StartLine) return `${f.File}:${f.StartLine}-${f.EndLine}`
  return `${f.File}:${f.StartLine}`
}
function shortHash(h) { return h && h.length > 7 ? h.slice(0, 7) : (h || '') }
function shortDate(d) { return d && d.length >= 10 ? d.slice(0, 10) : (d || '') }
function firstLine(s) {
  if (!s) return ''
  const i = s.indexOf('\n')
  return i >= 0 ? s.slice(0, i) : s
}
function entropyColor(e) {
  if (e >= 4) return 'var(--color-error)'
  if (e >= 3) return 'var(--color-warning)'
  return 'var(--color-text-tertiary)'
}
function maskedSecret(s) {
  if (!maskSecret.value) return s
  return s.length > 8 ? s.slice(0, 4) + '••••' + s.slice(-4) : '••••'
}
</script>

<template>
  <div class="sv-view">
    <!-- Top: toolbar -->
    <div class="sv-top">
      <a-button size="small" :loading="running" :disabled="!gitleaksStatus.available" @click="runScan">
        <template #icon><ScanOutlined /></template>扫描全历史
      </a-button>
      <a-tooltip title="关闭后显示完整密钥（默认脱敏）">
        <a-checkbox v-model:checked="maskSecret">脱敏密钥</a-checkbox>
      </a-tooltip>
      <span v-if="findingCount" class="sv-count">🚨 {{ findingCount }} 处泄露 · {{ fileGroups.length }} 个文件</span>
      <div class="sv-spacer"></div>
      <a-button size="small" type="text" @click="refreshTools" title="重新检测 gitleaks">
        <template #icon><SafetyCertificateOutlined /></template>检测
      </a-button>
    </div>

    <!-- gitleaks availability banner (only the "missing" case — the OK path
         shows version+path in the global footer instead, no need to repeat) -->
    <a-alert v-if="!gitleaksStatus.available" type="warning" show-icon message="未检测到 gitleaks">
      <template #description>
        <div>{{ gitleaksStatus.message || '请先安装 gitleaks' }}</div>
        <div class="sv-install">
          安装方式:<br />
          <code>go install github.com/zricethezav/gitleaks/v8@latest</code><br />
          或从 <a href="https://github.com/gitleaks/gitleaks/releases" target="_blank">GitHub Releases</a> 下载，并在设置中指定路径
        </div>
      </template>
    </a-alert>

    <!-- Empty / running -->
    <div v-if="!result && !running" class="sv-empty">
      <div class="sv-empty-icon"><KeyOutlined /></div>
      <div class="sv-empty-title">密钥泄露扫描</div>
      <div class="sv-empty-desc">扫描 Git 全历史中的 API Key / Token / 私钥等敏感信息泄露</div>
      <a-button size="large" type="primary" :disabled="!gitleaksStatus.available" @click="runScan">
        <template #icon><ScanOutlined /></template>开始扫描全历史
      </a-button>
      <div class="sv-features">
        <div class="sv-feature">
          <HistoryOutlined class="sf-icon" />
          <div class="sf-text"><b>全历史</b><span>遍历所有提交</span></div>
        </div>
        <div class="sv-feature">
          <SafetyCertificateOutlined class="sf-icon" />
          <div class="sf-text"><b>gitleaks 规则</b><span>内置大量检测</span></div>
        </div>
        <div class="sv-feature">
          <KeyOutlined class="sf-icon" />
          <div class="sf-text"><b>默认脱敏</b><span>密钥自动遮蔽</span></div>
        </div>
      </div>
    </div>
    <div v-if="running" class="sv-empty"><a-spin tip="扫描中，遍历全历史提交..." /></div>

    <!-- Result: clean -->
    <div v-if="result && !running && !findingCount" class="sv-empty">
      <CheckCircleOutlined style="font-size:40px;color:var(--color-success)" />
      <div class="sv-empty-title" style="color:var(--color-success)">未发现密钥泄露</div>
    </div>

    <!-- Result: stats overview -->
    <div v-if="result && !running && findingCount" class="sv-stats">
      <div class="sv-stat-card danger">
        <div class="sv-stat-num">{{ findingCount }}</div>
        <div class="sv-stat-label">处泄露</div>
      </div>
      <div class="sv-stat-card">
        <div class="sv-stat-num">{{ fileGroups.length }}</div>
        <div class="sv-stat-label">个文件</div>
      </div>
      <div class="sv-stat-card">
        <div class="sv-stat-num">{{ uniqueRuleCount }}</div>
        <div class="sv-stat-label">类规则</div>
      </div>
      <div class="sv-stat-card">
        <div class="sv-stat-num">{{ highEntropyCount }}</div>
        <div class="sv-stat-label">高熵 (≥4)</div>
      </div>
      <div v-if="ruleStats.length" class="sv-rule-dist">
        <span class="sv-rule-title">命中规则</span>
        <a-tag v-for="r in ruleStats.slice(0, 8)" :key="r.rule" color="red">{{ r.rule }} · {{ r.count }}</a-tag>
      </div>
    </div>

    <!-- Result: two columns -->
    <div v-if="result && !running && findingCount" class="sv-cols">
      <!-- Left: files with leaks -->
      <aside class="sv-list">
        <div class="sv-list-head">
          <span>泄露文件</span>
          <span>{{ fileGroups.length }} 文件 / {{ findingCount }} 处</span>
        </div>
        <div class="sv-list-body">
          <div
            v-for="g in fileGroups"
            :key="g.file"
            class="sv-item"
            :class="['sev-' + groupSeverityClass(g), { active: g.file === activeFile }]"
            @click="selectFile(g.file)"
          >
            <WarningOutlined class="sv-flag" />
            <div class="sv-item-body">
              <div class="sv-path" :title="g.file">{{ g.file }}</div>
              <div class="sv-item-meta">
                <span class="sv-cnt">{{ g.findings.length }} 处</span>
                <div class="sv-item-rules">
                  <a-tag v-for="r in groupRules(g)" :key="r">{{ r }}</a-tag>
                </div>
              </div>
            </div>
          </div>
        </div>
      </aside>

      <!-- Right: tabs (detail / source / resolve) -->
      <section class="sv-detail">
        <a-tabs v-model:activeKey="activeTab" size="small" class="sv-tabs">
          <!-- Tab 1: leak detail -->
          <a-tab-pane key="detail">
            <template #tab><span><WarningOutlined /> 泄露详情</span></template>
            <div class="sv-tab-body sv-detail-body">
              <div v-for="(f, i) in activeFindings" :key="i" class="sv-finding">
                <div class="fd-head">
                  <span class="fd-loc">{{ locStr(f) }}</span>
                  <a-tag color="red">{{ f.RuleID }}</a-tag>
                  <span class="fd-entropy" :style="{ color: entropyColor(f.Entropy) }" title="Shannon 熵">
                    熵 {{ f.Entropy.toFixed(2) }}
                  </span>
                </div>
                <div v-if="f.Description" class="fd-desc">{{ f.Description }}</div>

                <div v-if="f.Secret" class="fd-secret-row">
                  <span class="fd-label">密钥</span>
                  <code class="fd-secret">{{ maskedSecret(f.Secret) }}</code>
                </div>
                <div v-if="f.Match && f.Match !== f.Secret" class="fd-match-row">
                  <span class="fd-label">匹配</span>
                  <code class="fd-match">{{ f.Match }}</code>
                </div>

                <div v-if="f.Commit || f.Author" class="fd-commit">
                  <template v-if="f.Author">
                    <span class="fd-author">{{ f.Author }}</span>
                    <span v-if="f.Email" class="fd-email">&lt;{{ f.Email }}&gt;</span>
                  </template>
                  <span v-if="f.Date" class="fd-date">{{ shortDate(f.Date) }}</span>
                  <span v-if="f.Commit" class="fd-hash">{{ shortHash(f.Commit) }}</span>
                  <span v-if="f.Message" class="fd-msg">{{ firstLine(f.Message) }}</span>
                </div>
                <div v-if="f.Tags && f.Tags.length" class="fd-tags">
                  <a-tag v-for="t in f.Tags" :key="t" size="small">{{ t }}</a-tag>
                </div>
              </div>

              <!-- prominent CTA at the bottom of detail -->
              <div class="fd-cta" @click="goResolve">
                <ExclamationCircleOutlined class="cta-icon" />
                <span class="cta-text">检测到 <b>{{ activeFindings.length }}</b> 处密钥泄露，可能已被窃取</span>
                <span class="cta-link">查看解决方案 <ArrowRightOutlined /></span>
              </div>
            </div>
          </a-tab-pane>

          <!-- Tab 2: source code -->
          <a-tab-pane key="source">
            <template #tab><span><CodeOutlined /> 源码</span></template>
            <div class="sv-tab-body sv-source-body">
              <div class="sv-source-path">
                <code>{{ activeFile }}</code>
                <span v-if="activeFindings[0]?.Commit" class="sv-source-commit">@ {{ shortHash(activeFindings[0].Commit) }}</span>
              </div>
              <div v-if="fileLoading" class="sv-source-state"><a-spin /></div>
              <img v-else-if="fileInfo.dataUrl" :src="fileInfo.dataUrl" style="max-width:100%;max-height:100%;object-fit:contain;padding:16px;display:block;margin:auto" />
              <div v-else-if="fileInfo.binary" class="sv-source-state">⚠️ 该文件是二进制文件，无法显示源码</div>
              <div v-else-if="fileInfo.notFound" class="sv-source-state">⚠️ 该文件在此 commit 中不存在（可能已被删除）</div>
              <CodeViewer
                v-else
                ref="codeViewerRef"
                :code="fileContent"
                :filename="activeFile"
                :highlight-lines="leakLines"
              />
            </div>
          </a-tab-pane>

          <!-- Tab 3: resolution -->
          <a-tab-pane key="resolve">
            <template #tab><span><ToolOutlined /> 解决方案</span></template>
            <SecretResolvePanel
              :active-file="activeFile"
              :finding-count="activeFindings.length"
              :finding="activeFindings[0]"
              :hint="hintCache[activeFindings[0]?.RuleID]"
              v-model:scrub-auto-backup="scrubAutoBackup"
              :resolve-busy="resolveBusy"
              @remove="doRemoveFromWorkingTree"
              @scrub="doScrubHistory"
            />
          </a-tab-pane>
        </a-tabs>
      </section>
    </div>
  </div>
</template>

<style scoped>
.sv-view {
  width: calc(100% + var(--spacing-lg) * 2);
  height: calc(100% + var(--spacing-lg) * 2);
  margin: calc(-1 * var(--spacing-lg));
  display: flex; flex-direction: column;
  min-height: 0;
  background: transparent;
}

/* toolbar */
.sv-top { display: flex; align-items: center; gap: var(--spacing-sm); flex-shrink: 0; height: 38px; padding: 0 var(--spacing-md); background: var(--color-surface); border-bottom: 1px solid var(--color-border); }
.sv-spacer { flex: 1; }
.sv-count { font-size: 12px; color: var(--color-error); font-weight: 600; }
.sv-install { margin-top: 6px; font-size: 12px; color: var(--color-text-secondary); line-height: 1.8; }
.sv-install code { font-family: 'Cascadia Code', monospace; font-size: 11px; color: var(--color-primary); background: var(--color-background); padding: 1px 4px; border-radius: 3px; }

/* empty */
.sv-empty { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 10px; color: var(--color-text-tertiary); }
.sv-empty-icon { width: 64px; height: 64px; border-radius: 50%; display: flex; align-items: center; justify-content: center; background: var(--color-primary-bg); color: var(--color-primary); font-size: 30px; margin-bottom: 4px; }
.sv-empty-title { font-size: 17px; font-weight: 600; color: var(--color-text); }
.sv-empty-desc { font-size: 13px; color: var(--color-text-tertiary); text-align: center; max-width: 360px; }
.sv-features { display: flex; gap: var(--spacing-md); margin-top: var(--spacing-md); flex-wrap: wrap; justify-content: center; }
.sv-feature { display: flex; align-items: center; gap: 8px; padding: 10px 14px; background: var(--color-surface); border: 1px solid var(--color-border); border-radius: var(--radius-md); min-width: 160px; }
.sf-icon { font-size: 18px; color: var(--color-primary); flex-shrink: 0; }
.sf-text { display: flex; flex-direction: column; gap: 1px; }
.sf-text b { font-size: 12px; color: var(--color-text); font-weight: 600; }
.sf-text span { font-size: 11px; color: var(--color-text-tertiary); }

/* result stats */
.sv-stats { display: flex; align-items: center; gap: var(--spacing-sm); flex-shrink: 0; padding: var(--spacing-sm) var(--spacing-md); background: var(--color-surface); border: 1px solid var(--color-border); border-radius: var(--radius-md); flex-wrap: wrap; }
.sv-stat-card { display: flex; flex-direction: column; align-items: center; padding: 2px 16px; border-right: 1px solid var(--color-border-light); }
.sv-stat-card:last-of-type { border-right: none; }
.sv-stat-card.danger .sv-stat-num { color: var(--color-error); }
.sv-stat-num { font-size: 20px; font-weight: 700; color: var(--color-text); font-family: 'Cascadia Code', monospace; line-height: 1.2; }
.sv-stat-label { font-size: 11px; color: var(--color-text-tertiary); margin-top: 2px; }
.sv-rule-dist { display: flex; align-items: center; gap: 6px; flex-wrap: wrap; margin-left: auto; }
.sv-rule-title { font-size: 12px; color: var(--color-text-tertiary); }

/* two columns */
.sv-cols { flex: 1; display: flex; gap: 1px; min-height: 0; background: var(--color-border); }

/* left list */
.sv-list { width: 280px; flex-shrink: 0; background: var(--color-surface); display: flex; flex-direction: column; }
.sv-list-head { display: flex; justify-content: space-between; height: 34px; padding: 0 var(--spacing-md); border-bottom: 1px solid var(--color-border); font-size: 12px; font-weight: 600; color: var(--color-text-secondary); flex-shrink: 0; }
.sv-list-body { flex: 1; overflow-y: auto; padding: var(--spacing-xs); }
.sv-item { display: flex; gap: 6px; padding: 8px 10px; border-radius: var(--radius-sm); cursor: pointer; border-left: 3px solid transparent; transition: background var(--transition-fast); }
.sv-item:hover { background: var(--color-hover); }
.sv-item.active { background: var(--color-primary-bg); border-left-color: var(--color-primary); }
.sv-item.sev-high { border-left-color: var(--color-error); }
.sv-item.sev-mid { border-left-color: var(--color-warning); }
.sv-flag { color: var(--color-error); font-size: 13px; margin-top: 2px; flex-shrink: 0; }
.sv-item.sev-mid .sv-flag { color: var(--color-warning); }
.sv-item.sev-low .sv-flag { color: var(--color-text-tertiary); }
.sv-item-body { flex: 1; min-width: 0; }
.sv-path { font-size: 12px; color: var(--color-text); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-family: 'Cascadia Code', monospace; }
.sv-item-meta { display: flex; align-items: center; gap: 6px; margin-top: 4px; flex-wrap: wrap; }
.sv-cnt { font-size: 10px; color: var(--color-text-tertiary); flex-shrink: 0; }
.sv-item-rules { display: flex; gap: 3px; flex-wrap: wrap; }
.sv-item-rules :deep(.ant-tag) { margin: 0; font-size: 9px; line-height: 14px; padding: 0 4px; }

/* right detail + tabs */
.sv-detail { flex: 1; min-width: 0; background: var(--color-surface); display: flex; flex-direction: column; overflow: hidden; }
.sv-tabs { flex: 1; display: flex; flex-direction: column; min-height: 0; }
.sv-tabs :deep(.ant-tabs-nav) { margin: 0; padding: 0 var(--spacing-sm); flex-shrink: 0; }
.sv-tabs :deep(.ant-tabs-content-holder) { flex: 1; min-height: 0; overflow: hidden; }
.sv-tabs :deep(.ant-tabs-content) { height: 100%; }
.sv-tabs :deep(.ant-tabs-tabpane) { height: 100%; display: flex; flex-direction: column; }
.sv-tab-body { flex: 1; min-height: 0; overflow-y: auto; padding: var(--spacing-sm) var(--spacing-md); }

/* finding card */
.sv-finding { margin-bottom: var(--spacing-md); padding: var(--spacing-sm); background: var(--color-background); border-radius: var(--radius-sm); border-left: 3px solid var(--color-error); }
.fd-head { display: flex; align-items: center; gap: 8px; margin-bottom: 6px; flex-wrap: wrap; }
.fd-loc { font-family: 'Cascadia Code', monospace; font-size: 12px; color: var(--color-primary); font-weight: 600; word-break: break-all; }
.fd-entropy { font-size: 11px; font-weight: 600; margin-left: auto; }
.fd-desc { font-size: 12px; color: var(--color-text-secondary); margin-bottom: 8px; line-height: 1.5; }
.fd-label { font-size: 11px; color: var(--color-text-tertiary); width: 32px; flex-shrink: 0; }
.fd-secret-row, .fd-match-row { display: flex; align-items: baseline; gap: 8px; margin-bottom: 4px; }
.fd-secret { font-family: 'Cascadia Code', monospace; font-size: 12px; color: var(--color-error); background: var(--color-error-bg); padding: 2px 6px; border-radius: var(--radius-xs); word-break: break-all; }
.fd-match { font-family: 'Cascadia Code', monospace; font-size: 11px; color: var(--color-text-secondary); word-break: break-all; }
.fd-commit { display: flex; align-items: center; gap: 6px; flex-wrap: wrap; margin-top: 8px; padding-top: 6px; border-top: 1px solid var(--color-border-light); font-size: 11px; color: var(--color-text-tertiary); }
.fd-author { color: var(--color-text-secondary); }
.fd-hash { font-family: 'Cascadia Code', monospace; color: var(--color-hash); }
.fd-msg { color: var(--color-text-tertiary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 300px; }
.fd-tags { display: flex; gap: 4px; margin-top: 6px; flex-wrap: wrap; }

/* prominent CTA at bottom of detail tab */
.fd-cta { display: flex; align-items: center; gap: 10px; margin-top: var(--spacing-md); padding: 12px 14px; background: var(--color-error-bg); border: 1px solid var(--color-error); border-radius: var(--radius-md); cursor: pointer; transition: all var(--transition-fast); }
.fd-cta:hover { background: rgba(255, 85, 85, 0.18); transform: translateY(-1px); box-shadow: var(--shadow-sm); }
.cta-icon { font-size: 18px; color: var(--color-error); flex-shrink: 0; }
.cta-text { flex: 1; font-size: 13px; color: var(--color-text); }
.cta-text b { color: var(--color-error); }
.cta-link { font-size: 13px; color: var(--color-primary); font-weight: 600; white-space: nowrap; }

/* resolution tab — step cards */
.sv-resolve-body { padding: var(--spacing-md) var(--spacing-md); }
.rv-header { margin-bottom: var(--spacing-md); padding-bottom: var(--spacing-sm); border-bottom: 1px solid var(--color-border-light); }
.rv-file { font-family: 'Cascadia Code', monospace; font-size: 13px; color: var(--color-text); font-weight: 600; }
.rv-sub { margin-left: 8px; font-size: 12px; color: var(--color-text-tertiary); }

.rv-step { margin-bottom: var(--spacing-md); border: 1px solid var(--color-border); border-radius: var(--radius-md); overflow: hidden; background: var(--color-surface); }
.rv-step-revoke { border-color: var(--color-error); }
.rv-step-remove { border-color: var(--color-warning); }
.rv-step-scrub { border-color: var(--color-error); }

.rv-step-head { display: flex; align-items: center; gap: 10px; padding: 10px 14px; font-weight: 600; font-size: 13px; }
.rv-step-revoke .rv-step-head { background: var(--color-error-bg); color: var(--color-error); }
.rv-step-remove .rv-step-head { background: var(--color-warning-bg); color: var(--color-warning); }
.rv-step-scrub .rv-step-head { background: var(--color-error-bg); color: var(--color-error); }

.rv-step-num { display: inline-flex; align-items: center; justify-content: center; width: 22px; height: 22px; border-radius: 50%; font-size: 12px; font-weight: 700; flex-shrink: 0; }
.rv-step-revoke .rv-step-num { background: var(--color-error); color: #fff; }
.rv-step-remove .rv-step-num { background: var(--color-warning); color: #fff; }
.rv-step-scrub .rv-step-num { background: var(--color-error); color: #fff; }
.rv-step-title { flex: 1; display: flex; align-items: center; gap: 6px; }
.rv-must { margin-left: 0; }

.rv-step-body { padding: 12px 14px; }
.rv-explain { font-size: 12px; color: var(--color-text-secondary); line-height: 1.6; margin: 0 0 8px 0; }
.rv-explain code, .rv-warn code, .rv-warn-list code, .rv-note code { font-family: 'Cascadia Code', monospace; font-size: 11px; color: var(--color-primary); background: var(--color-background); padding: 1px 4px; border-radius: 3px; }
.rv-warn { font-size: 12px; color: var(--color-warning); margin: 0 0 10px 0; }
.rv-warn-list { font-size: 12px; color: var(--color-text-secondary); line-height: 1.8; margin: 0 0 10px 0; padding-left: 18px; }
.rv-note { font-size: 12px; color: var(--color-success); margin: 0 0 10px 0; }

.rv-hint-box { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; padding: 8px 10px; background: var(--color-background); border-radius: var(--radius-sm); margin-bottom: 10px; }
.rv-hint-label { font-size: 11px; color: var(--color-text-tertiary); flex-shrink: 0; }
.rv-hint-text { font-size: 12px; color: var(--color-text); flex: 1; }
.rv-open-btn { padding: 0 4px !important; height: auto !important; font-size: 12px; }

/* source view */
.sv-source-body { display: flex; flex-direction: column; padding: 0 !important; }
.sv-source-path { display: flex; align-items: center; gap: 8px; padding: var(--spacing-xs) var(--spacing-md); border-bottom: 1px solid var(--color-border-light); flex-shrink: 0; font-size: 12px; }
.sv-source-path code { font-family: 'Cascadia Code', monospace; color: var(--color-text); word-break: break-all; }
.sv-source-commit { font-family: 'Cascadia Code', monospace; color: var(--color-hash); flex-shrink: 0; }
.sv-source-state { flex: 1; display: flex; align-items: center; justify-content: center; color: var(--color-text-tertiary); font-size: 13px; }
.sv-source-body :deep(.cv-host) { flex: 1; min-height: 0; }
</style>
