<script setup>
import { ref, computed, onMounted } from 'vue'
import { message as AMessage, Modal } from 'ant-design-vue'
import {
  FileTextOutlined, ReloadOutlined, PlusOutlined, SearchOutlined,
  ThunderboltOutlined, CheckCircleOutlined, SafetyCertificateOutlined
} from '@ant-design/icons-vue'
import CodeViewer from '../../../components/common/CodeViewer.vue'

const app = window.go?.main?.ByteGitApp

// ---- current LICENSE content ----
const currentContent = ref('')
const contentLoading = ref(false)
const dirty = ref(false)
const saving = ref(false)

async function loadLicense() {
  contentLoading.value = true
  dirty.value = false
  try {
    const res = await app.LicenseRead()
    currentContent.value = res.notFound ? '' : res.content
  } catch { currentContent.value = '' }
  finally { contentLoading.value = false }
}

function onContentChange(v) { currentContent.value = v; dirty.value = true }
async function saveLicense() {
  if (!dirty.value) return
  saving.value = true
  try { await app.LicenseWrite(currentContent.value); dirty.value = false; AMessage.success('已保存') }
  catch (e) { AMessage.error('保存失败') }
  finally { saving.value = false }
}
function onKeydown(e) { if ((e.ctrlKey||e.metaKey) && e.key==='s') { e.preventDefault(); saveLicense() } }

// ---- detection result (badge in header) ----
const detection = ref(null)

async function runDetect() {
  try {
    detection.value = await app.LicenseDetect()
  } catch { detection.value = null }
}

// ---- license templates ----
const templates = ref([])
const createOpen = ref(false)
const createSearch = ref('')
const createSelected = ref(null)
const createAuthor = ref('')
const createYear = ref('')

const filteredTemplates = computed(() => {
  if (!createSearch.value.trim()) return templates.value
  const q = createSearch.value.toLowerCase()
  return templates.value.filter(t => t.spdxId.toLowerCase().includes(q) || t.title.toLowerCase().includes(q))
})

async function openCreate() {
  createOpen.value = true
  createSearch.value = ''
  createSelected.value = null
  if (!templates.value.length) {
    try { templates.value = await app.LicenseListTemplates() } catch {}
  }
}

async function doCreate() {
  if (!createSelected.value) return
  try {
    const content = await app.LicenseCreate(createSelected.value.spdxId, createAuthor.value, createYear.value)
    currentContent.value = content
    dirty.value = true
    await saveLicense()
    createOpen.value = false
    await runDetect()
    AMessage.success(`已创建 ${createSelected.value.spdxId} 许可证`)
  } catch (e) { AMessage.error('创建失败: ' + String(e?.message || e)) }
}

// ---- audit (dependency licenses) ----
const auditOpen = ref(false)
const auditResults = ref(null)
const auditLoading = ref(false)

async function openAudit() {
  auditOpen.value = true
  auditResults.value = null
  auditLoading.value = true
  try { auditResults.value = await app.LicenseAuditDeps() }
  catch (e) { AMessage.error('审查失败') }
  finally { auditLoading.value = false }
}

const riskEmoji = { low: '🟢', medium: '🟡', high: '🔴', unknown: '⚪' }
const riskName = { low: '低风险', medium: '中风险', high: '高风险', unknown: '未知' }
const riskCounts = computed(() => {
  if (!auditResults.value) return {}
  const c = { low: 0, medium: 0, high: 0, unknown: 0 }
  for (const e of auditResults.value) c[e.riskLevel] = (c[e.riskLevel] || 0) + 1
  return c
})

onMounted(async () => {
  await loadLicense()
  await runDetect()
})
</script>

<template>
  <div class="lic-page" @keydown="onKeydown">
    <!-- header bar -->
    <div class="lic-bar">
      <span class="lic-bar-title"><SafetyCertificateOutlined /> LICENSE</span>
      <!-- detection badge -->
      <template v-if="detection && detection.isValid">
        <a-tag color="green">{{ detection.spdxId }}</a-tag>
        <span class="lic-conf">{{ (detection.confidence * 100).toFixed(0) }}%</span>
      </template>
      <a-tag v-else-if="currentContent" color="orange">未识别</a-tag>
      <span v-if="currentContent" class="lic-bar-lines">{{ currentContent.split('\n').length }} 行</span>
      <span class="lic-bar-hint">右键打开操作菜单</span>
      <div class="lic-bar-spacer" />
      <a-button size="small" type="text" @click="loadLicense(); runDetect()" :loading="contentLoading"><ReloadOutlined /></a-button>
    </div>

    <!-- main: CodeViewer with context menu -->
    <a-dropdown :trigger="['contextmenu']" placement="bottomLeft">
      <div class="lic-canvas">
        <div v-if="contentLoading" class="lic-center"><a-spin /></div>
        <div v-else-if="currentContent" class="lic-edit-wrap" @keydown="onKeydown">
          <CodeViewer :code="currentContent" filename="LICENSE" :highlight-lines="[]" editable @update:modelValue="onContentChange" :show-context-menu="false" />
          <div v-if="dirty" class="lic-save-bar">
            <span class="lic-dirty">● 未保存</span>
            <div class="lic-save-spacer" />
            <a-button size="small" :loading="saving" @click="saveLicense">保存 (Ctrl+S)</a-button>
          </div>
        </div>
        <!-- empty: no LICENSE -->
        <div v-else class="lic-center">
          <SafetyCertificateOutlined style="font-size:40px;opacity:0.25" />
          <div style="margin-top:8px">无 LICENSE 文件</div>
          <a-button type="primary" size="small" style="margin-top:12px" @click="openCreate">
            <PlusOutlined /> 创建许可证
          </a-button>
        </div>
      </div>

      <!-- context menu -->
      <template #overlay>
        <a-menu>
          <!-- create (only when no LICENSE exists) -->
          <a-menu-item v-if="!currentContent" key="create" @click="openCreate">
            <template #icon><PlusOutlined /></template>
            创建许可证
          </a-menu-item>
          <!-- detect -->
          <a-menu-item key="detect" @click="runDetect">
            <template #icon><SearchOutlined /></template>
            检测许可证
          </a-menu-item>
          <!-- audit deps -->
          <a-menu-item key="audit" @click="openAudit">
            <template #icon><SafetyCertificateOutlined /></template>
            依赖许可证审查
          </a-menu-item>
          <a-menu-divider />
          <a-menu-item key="reload" @click="loadLicense(); runDetect()">
            <template #icon><ReloadOutlined /></template>
            刷新
          </a-menu-item>
        </a-menu>
      </template>
    </a-dropdown>

    <!-- create license modal -->
    <a-modal v-model:open="createOpen" title="创建许可证" width="640px" :footer="null" destroy-on-close>
      <div class="lc-modal">
        <a-input v-model:value="createSearch" placeholder="搜索许可证…" size="small" allow-clear style="margin-bottom:12px" />
        <div class="lc-list">
          <div v-for="t in filteredTemplates" :key="t.spdxId"
            class="lc-item" :class="{ selected: createSelected?.spdxId === t.spdxId, featured: t.featured }"
            @click="createSelected = t">
            <div class="lc-item-main">
              <span class="lc-item-id">{{ t.spdxId }}</span>
              <span v-if="t.featured" class="lc-star">★</span>
              <span class="lc-item-title">{{ t.title }}</span>
            </div>
            <div class="lc-item-cat" :class="'cat-' + t.category">{{ t.category }}</div>
          </div>
        </div>
        <!-- author + year -->
        <div v-if="createSelected" class="lc-form">
          <div class="lc-form-row">
            <label>作者</label>
            <input class="lc-input" v-model="createAuthor" placeholder="cicbyte" />
          </div>
          <div class="lc-form-row">
            <label>年份</label>
            <input class="lc-input" v-model="createYear" placeholder="2026（默认当前年份）" />
          </div>
          <!-- permissions/conditions/limitations -->
          <div class="lc-meta">
            <div v-if="createSelected.permissions.length" class="lc-meta-row">
              <span class="lc-meta-k">允许</span>
              <a-tag v-for="p in createSelected.permissions" :key="p" color="green" size="small">{{ p }}</a-tag>
            </div>
            <div v-if="createSelected.conditions.length" class="lc-meta-row">
              <span class="lc-meta-k">条件</span>
              <a-tag v-for="c in createSelected.conditions" :key="c" color="blue" size="small">{{ c }}</a-tag>
            </div>
            <div v-if="createSelected.limitations.length" class="lc-meta-row">
              <span class="lc-meta-k">限制</span>
              <a-tag v-for="l in createSelected.limitations" :key="l" color="red" size="small">{{ l }}</a-tag>
            </div>
          </div>
          <div class="lc-form-actions">
            <a-button size="small" @click="createOpen = false">取消</a-button>
            <a-button size="small" type="primary" @click="doCreate">
              <CheckCircleOutlined /> 创建 LICENSE
            </a-button>
          </div>
        </div>
      </div>
    </a-modal>

    <!-- audit modal -->
    <a-modal v-model:open="auditOpen" title="依赖许可证审查" width="600px" :footer="null" destroy-on-close>
      <div class="ad-modal">
        <div v-if="auditLoading" class="ad-loading"><a-spin tip="扫描中…" /></div>
        <template v-else-if="auditResults">
          <div class="ad-summary">
            <span class="ad-count">🟢 {{ riskCounts.low }} 低</span>
            <span class="ad-count">🟡 {{ riskCounts.medium }} 中</span>
            <span class="ad-count">🔴 {{ riskCounts.high }} 高</span>
            <span class="ad-count">⚪ {{ riskCounts.unknown }} 未知</span>
          </div>
          <div class="ad-list">
            <div v-for="(e, i) in auditResults" :key="i" class="ad-row" :class="'risk-' + e.riskLevel">
              <span class="ad-risk">{{ riskEmoji[e.riskLevel] }}</span>
              <span class="ad-spdx">{{ e.spdxId || '未识别' }}</span>
              <span class="ad-path">{{ e.path }}</span>
            </div>
          </div>
        </template>
        <div v-else class="ad-empty">无结果</div>
      </div>
    </a-modal>
  </div>
</template>

<style scoped>
.lic-page { width: calc(100% + var(--spacing-lg) * 2); height: calc(100% + var(--spacing-lg) * 2); margin: calc(-1 * var(--spacing-lg)); display: flex; flex-direction: column; min-height: 0; background: transparent; }
.lic-bar { display: flex; align-items: center; gap: 8px; padding: 0 12px; height: 34px; border-bottom: 1px solid var(--color-border); flex-shrink: 0; }
.lic-bar-title { font-size: 13px; font-weight: 600; color: var(--color-text); display: flex; align-items: center; gap: 6px; }
.lic-conf { font-size: 11px; color: var(--color-success); font-weight: 600; }
.lic-bar-lines { font-size: 11px; color: var(--color-text-tertiary); }
.lic-bar-hint { font-size: 10px; color: var(--color-text-tertiary); opacity: 0.6; }
.lic-bar-spacer { flex: 1; }
.lic-canvas { flex: 1; min-height: 0; overflow: hidden; position: relative; }
.lic-edit-wrap { height: 100%; display: flex; flex-direction: column; position: relative; }
.lic-edit-wrap :deep(.cv-host) { flex: 1; min-height: 0; }
.lic-save-bar { display: flex; align-items: center; gap: 8px; padding: 4px 12px; border-top: 1px solid var(--color-border); background: var(--color-warning-bg); flex-shrink: 0; }
.lic-dirty { font-size: 11px; color: var(--color-warning); font-weight: 600; }
.lic-save-spacer { flex: 1; }
.lic-center { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; height: 100%; gap: 8px; color: var(--color-text-tertiary); font-size: 13px; }

/* create modal */
.lc-modal { max-height: 65vh; display: flex; flex-direction: column; }
.lc-list { flex: 1; overflow-y: auto; border: 1px solid var(--color-border-light); border-radius: var(--radius-sm); margin-bottom: 12px; max-height: 300px; }
.lc-item { display: flex; align-items: center; justify-content: space-between; padding: 8px 12px; cursor: pointer; transition: background var(--transition-fast); gap: 8px; }
.lc-item:hover { background: var(--color-hover); }
.lc-item.selected { background: var(--color-primary-bg); }
.lc-item.featured { border-left: 3px solid var(--color-primary); }
.lc-item-main { display: flex; align-items: center; gap: 6px; flex: 1; min-width: 0; }
.lc-item-id { font-family: 'Cascadia Code', monospace; font-size: 13px; font-weight: 600; color: var(--color-text); }
.lc-star { color: var(--color-warning); font-size: 12px; }
.lc-item-title { font-size: 12px; color: var(--color-text-tertiary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.lc-item-cat { font-size: 10px; padding: 1px 6px; border-radius: var(--radius-xs); flex-shrink: 0; text-transform: uppercase; }
.cat-permissive { color: var(--color-success); background: var(--color-success-bg); }
.cat-weak-copyleft { color: var(--color-warning); background: var(--color-warning-bg); }
.cat-strong-copyleft { color: var(--color-error); background: var(--color-error-bg); }
.cat-public-domain { color: var(--color-text-tertiary); background: var(--color-surface-raised); }
.lc-form { border-top: 1px solid var(--color-border); padding-top: 12px; }
.lc-form-row { display: flex; align-items: center; gap: 8px; margin-bottom: 8px; }
.lc-form-row label { font-size: 12px; color: var(--color-text-secondary); width: 40px; flex-shrink: 0; }
.lc-input { flex: 1; padding: 5px 10px; font-size: 13px; border: 1px solid var(--color-border); border-radius: var(--radius-xs); background: var(--color-background); color: var(--color-text); outline: none; }
.lc-input:focus { border-color: var(--color-primary); }
.lc-meta { margin: 8px 0; display: flex; flex-direction: column; gap: 4px; }
.lc-meta-row { display: flex; align-items: center; gap: 4px; flex-wrap: wrap; }
.lc-meta-k { font-size: 11px; color: var(--color-text-tertiary); width: 32px; flex-shrink: 0; }
.lc-form-actions { display: flex; justify-content: flex-end; gap: 8px; }

/* audit modal */
.ad-modal { display: flex; flex-direction: column; gap: var(--spacing-sm); min-height: 300px; }
.ad-loading { flex: 1; display: flex; align-items: center; justify-content: center; }
.ad-summary { display: flex; gap: 12px; padding: 8px 0; font-size: 13px; }
.ad-count { font-weight: 600; }
.ad-list { flex: 1; overflow-y: auto; max-height: 400px; }
.ad-row { display: flex; align-items: center; gap: 8px; padding: 5px 10px; border-radius: var(--radius-xs); font-size: 12px; }
.ad-row.risk-high { background: var(--color-error-bg); }
.ad-row.risk-medium { background: var(--color-warning-bg); }
.ad-risk { font-size: 14px; flex-shrink: 0; }
.ad-spdx { font-family: 'Cascadia Code', monospace; font-weight: 600; color: var(--color-text); min-width: 100px; flex-shrink: 0; }
.ad-path { color: var(--color-text-tertiary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.ad-empty { padding: 40px; text-align: center; color: var(--color-text-tertiary); }
</style>
