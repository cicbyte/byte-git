<script setup>
import { ref, computed, onMounted, watch } from 'vue'
import { message as AMessage, Modal } from 'ant-design-vue'
import {
  SaveOutlined, ReloadOutlined, UndoOutlined, TableOutlined, CodeOutlined,
  EyeOutlined, DeleteOutlined,
} from '@ant-design/icons-vue'
import { useRepoStore } from '@/stores/repo'
import { EventsOn, EventsOff } from '@bridge/runtime/runtime'
import CodeViewer from '@/components/common/CodeViewer.vue'
import RewriteRowEditor from '@/components/rewrite/RewriteRowEditor.vue'
import RewritePreviewModal from '@/components/rewrite/RewritePreviewModal.vue'

/**
 * 改写数据编辑器 — 读写固定路径 rewrite.json。
 *
 * 两种视图，顶部切换，共享同一份数据：
 *   - 表单模式（默认）：表格 + 行内展开编辑（原始 vs 改写对比）
 *   - JSON 模式：全屏 CodeMirror 直接编辑原始 JSON
 *
 * 切换时双向同步：表单→JSON 序列化；JSON→表单解析回 rows。
 */
const repo = useRepoStore()
const app = window.go?.main?.ByteGitApp

// ---- view mode ----
const viewMode = ref('table')   // 'table' | 'json'

// ---- data ----
const rows = ref([])          // MetaEntryDTO[]
const loading = ref(false)
const dirty = ref(false)      // 有未保存改动
const filePath = ref('')
const fileExists = ref(false)
const expandedRowKeys = ref([])

// JSON 模式的文本缓冲（切到 json 模式时从 rows 同步）
const jsonText = ref('')

function isChanged(row) {
  const o = row.original, r = row.rewrite
  return o.authorName !== r.authorName ||
    o.authorEmail !== r.authorEmail ||
    o.authorDate !== r.authorDate ||
    o.committerName !== r.committerName ||
    o.committerEmail !== r.committerEmail ||
    o.committerDate !== r.committerDate ||
    o.message !== r.message
}
function fieldChanged(row, key) { return row.original[key] !== row.rewrite[key] }
function changedFieldNames(row) {
  const all = [
    ['作者名', 'authorName'], ['作者邮箱', 'authorEmail'], ['作者日期', 'authorDate'],
    ['提交者名', 'committerName'], ['提交者邮箱', 'committerEmail'], ['提交者日期', 'committerDate'],
    ['message', 'message'],
  ]
  return all.filter(([, k]) => row.original[k] !== row.rewrite[k]).map(([n]) => n)
}

const changedCount = computed(() => rows.value.filter(isChanged).length)
const hasData = computed(() => rows.value.length > 0)

function markDirty() { dirty.value = true }

// hash → 全局行号（1 起）：行号列必须显示全局序，
// 与校验消息的"第 N 行"保持一致。
const rowIndexOf = computed(() => {
  const m = new Map()
  rows.value.forEach((r, i) => m.set(r.hash, i + 1))
  return m
})

// 通过 hash 找回 rows.value 里的原始对象引用，确保 v-model 改动可靠写入 rows。
// antd table 的 record 在某些情况下可能是代理/副本，直接绑定 record.rewrite 不一定
// 回写到 dataSource。这里用 hash 定位原对象，编辑面板绑定到它。
function getRow(hash) {
  return rows.value.find((r) => r.hash === hash)
}

// ---- mode switching (table <-> json) ----
function switchMode(mode) {
  if (mode === viewMode.value) return
  if (mode === 'json') {
    // table → json：序列化当前 rows
    jsonText.value = JSON.stringify(rows.value, null, 2)
  } else {
    // json → table：解析 jsonText 回 rows；失败则不切换
    try {
      const parsed = JSON.parse(jsonText.value)
      if (!Array.isArray(parsed)) {
        AMessage.error('JSON 顶层必须是数组')
        return
      }
      rows.value = parsed
    } catch (e) {
      AMessage.error('JSON 格式错误，无法切回表单: ' + String(e?.message || e))
      return
    }
  }
  viewMode.value = mode
}

function onJsonEdit(v) {
  jsonText.value = v
  markDirty()
}

// ---- table columns ----
const columns = [
  { title: '#', key: 'idx', width: 50, align: 'center' },
  { title: '', key: 'status', width: 30, align: 'center' },
  { title: 'Hash', key: 'hash', width: 90 },
  { title: '作者', key: 'authorName', width: 120, ellipsis: true },
  { title: '邮箱', key: 'authorEmail', width: 160, ellipsis: true },
  { title: '日期', key: 'authorDate', width: 150, ellipsis: true },
  { title: 'message', key: 'subject', ellipsis: true },
  { title: '推送', key: 'pushed', width: 60, align: 'center' },
]

// ---- load / save ----
async function loadPath() {
  try { filePath.value = await app.JsonEditorPath() } catch { /* ignore */ }
}

async function loadFromDisk() {
  if (!app?.JsonEditorRead) return
  loading.value = true
  try {
    const content = await app.JsonEditorRead()
    if (content) {
      const parsed = JSON.parse(content)
      if (Array.isArray(parsed)) {
        rows.value = parsed
        fileExists.value = true
      } else {
        rows.value = []
        fileExists.value = false
      }
    } else {
      rows.value = []
      fileExists.value = false
    }
    dirty.value = false
    expandedRowKeys.value = []
    jsonText.value = JSON.stringify(rows.value, null, 2)
  } catch (e) {
    AMessage.error('读取失败: ' + String(e?.message || e))
    rows.value = []
  } finally {
    loading.value = false
  }
}

// ---- 校验 ----
// 返回错误消息数组（空数组=通过）。校验 rewrite 字段间的合理性。
function validateRows(list) {
  const errs = []

  // 1. 逐行校验：作者与提交者必须一致（名/邮箱/日期分别相等）；日期格式有效
  for (let i = 0; i < list.length; i++) {
    const r = list[i].rewrite
    const idx = `第 ${i + 1} 行`

    // 日期格式校验
    for (const [label, val] of [['作者日期', r.authorDate], ['提交者日期', r.committerDate]]) {
      if (val && isNaN(new Date(val).getTime())) {
        errs.push(`${idx}: ${label}「${val}」不是有效的日期`)
      }
    }

    // 作者 = 提交者 必须一致
    if (r.authorName !== r.committerName) {
      errs.push(`${idx}: 作者名(${r.authorName}) 与提交者名(${r.committerName}) 不一致`)
    }
    if (r.authorEmail !== r.committerEmail) {
      errs.push(`${idx}: 作者邮箱(${r.authorEmail}) 与提交者邮箱(${r.committerEmail}) 不一致`)
    }
    if (r.authorDate !== r.committerDate) {
      errs.push(`${idx}: 作者日期(${r.authorDate}) 与提交者日期(${r.committerDate}) 不一致`)
    }
  }

  // 2. 整棵树日期单调递增：后面的提交日期不能早于前面的
  //    rows 的顺序就是 git log 顺序（新→旧），所以反序遍历（旧→新），
  //    每条的时间不应早于前一条（更旧的）。
  let prevTime = null
  let prevIdx = -1
  for (let i = list.length - 1; i >= 0; i--) {
    const dateStr = list[i].rewrite.authorDate
    if (!dateStr) continue
    const t = new Date(dateStr).getTime()
    if (isNaN(t)) continue
    if (prevTime !== null && t < prevTime) {
      errs.push(`第 ${i + 1} 行日期(${dateStr}) 早于第 ${prevIdx + 1} 行(${list[prevIdx].rewrite.authorDate})，提交时间应单调递增`)
    }
    prevTime = t
    prevIdx = i
  }

  return errs
}

async function saveFile() {
  try {
    // 若当前在 JSON 模式，先同步回 rows 再校验保存
    let toSave = rows.value
    if (viewMode.value === 'json') {
      const parsed = JSON.parse(jsonText.value)
      if (!Array.isArray(parsed)) {
        AMessage.error('JSON 顶层必须是数组')
        return
      }
      toSave = parsed
      rows.value = parsed
    }
    for (const r of toSave) {
      if (!r.hash || !r.original || !r.rewrite) {
        AMessage.error('数据不完整：存在缺少 hash/original/rewrite 的行')
        return
      }
    }
    // 字段校验
    const errs = validateRows(toSave)
    if (errs.length) {
      // 显示前 5 条错误
      Modal.error({
        title: `校验失败（${errs.length} 处问题）`,
        content: errs.slice(0, 8).map((e) => '· ' + e).join('\n') + (errs.length > 8 ? `\n…等 ${errs.length} 处` : ''),
        okText: '去修改',
      })
      return
    }
    const text = JSON.stringify(toSave, null, 2)
    await app.JsonEditorWrite(text)
    dirty.value = false
    fileExists.value = true
    AMessage.success(`已保存 ${toSave.length} 条`)
  } catch (e) {
    AMessage.error('保存失败: ' + String(e?.message || e))
  }
}

function restoreRow(row) {
  row.rewrite = JSON.parse(JSON.stringify(row.original))
  markDirty()
}

// 单行 AI 整理 message：走 RewriteGenerate（使用「提示词」页的改写规则），
// 订阅 rewrite:token 流式回显到改写值输入框，完成后以清洗结果定稿；
// diff 由后端按 hash 自动加载；行级互斥防重复触发
const aiLoadingHash = ref('')
async function aiMessage(row) {
  if (!app?.RewriteGenerate || aiLoadingHash.value) return
  aiLoadingHash.value = row.hash
  let buf = ''
  EventsOn('rewrite:token', (d) => {
    if (!d || d.hash !== row.hash) return
    buf += d.token || ''
    row.rewrite.message = buf
    markDirty()
  })
  try {
    const suggested = await app.RewriteGenerate(repo.repoArg(), {
      hash: row.hash,
      original: row.original.message,
      files: [],
      diffSummary: '',
      diff: '',
    })
    if (suggested && suggested.trim()) {
      row.rewrite.message = suggested.trim()
    } else if (buf.trim()) {
      row.rewrite.message = buf.trim() // 清洗结果缺失时保留已流出的原文
    } else {
      AMessage.warning('AI 未返回内容')
    }
  } catch (e) {
    if (buf.trim()) row.rewrite.message = buf.trim() // 失败时保留已生成部分供手改
    AMessage.error('AI 整理失败: ' + String(e?.message || e))
  } finally {
    EventsOff('rewrite:token')
    aiLoadingHash.value = ''
  }
}
function restoreAll() {
  rows.value.forEach((r) => { r.rewrite = JSON.parse(JSON.stringify(r.original)) })
  markDirty()
  AMessage.success('已全部恢复原值')
}

// remove the on-disk rewrite.json (the in-memory data stays until reload/close)
function removeFile() {
  Modal.confirm({
    title: '删除 rewrite.json？',
    content: '将删除磁盘上的改写数据文件（.git/byte-git/rewrite/rewrite.json）。内存中的当前数据会保留，但下次进入或切换仓库后将清空。此操作不可撤销。',
    okText: '删除',
    okType: 'danger',
    cancelText: '取消',
    async onOk() {
      try {
        await app.JsonEditorRemove()
        fileExists.value = false
        rows.value = []
        jsonText.value = ''
        dirty.value = false
        expandedRowKeys.value = []
        AMessage.success('已删除 rewrite.json')
      } catch (e) {
        AMessage.error('删除失败: ' + String(e?.message || e))
      }
    },
  })
}

// ---- preview (dry-run) ----
const previewVisible = ref(false)

function openPreview() {
  if (!changedCount.value) {
    AMessage.warning('没有需要改写的提交')
    return
  }
  previewVisible.value = true
}

// ---- lifecycle ----
onMounted(async () => {
  await loadPath()
  await loadFromDisk()
})
watch(() => repo.repoEpoch, async () => {
  dirty.value = false
  await loadPath()
  await loadFromDisk()
})
</script>

<template>
  <div class="rde-view">
    <!-- toolbar -->
    <div class="rde-toolbar">
      <div class="rde-tb-left">
        <span class="rde-title">数据编辑器</span>
        <!-- mode switch (left, so right-side action buttons don't shift) -->
        <a-radio-group v-model:value="viewMode" size="small" button-style="solid" @change="(e) => switchMode(e.target.value)">
          <a-radio-button value="table"><TableOutlined /> 表单</a-radio-button>
          <a-radio-button value="json"><CodeOutlined /> JSON</a-radio-button>
        </a-radio-group>
        <a-divider type="vertical" />
        <span v-if="hasData" class="rde-stat">共 <b>{{ rows.length }}</b> 条 · 已改 <b class="rde-changed">{{ changedCount }}</b></span>
        <a-tag v-if="fileExists" color="green">已加载</a-tag>
        <a-tag v-else color="default">未导出</a-tag>
        <span v-if="dirty" class="rde-dirty">未保存 *</span>
      </div>
      <div class="rde-tb-right">
        <a-button size="small" @click="loadFromDisk" :loading="loading" :disabled="!fileExists && !hasData" title="从文件重新加载">
          <template #icon><ReloadOutlined /></template>重新加载
        </a-button>
        <a-button size="small" danger @click="removeFile" :disabled="!fileExists" title="删除磁盘上的 rewrite.json">
          <template #icon><DeleteOutlined /></template>删除文件
        </a-button>
        <a-button size="small" @click="restoreAll" :disabled="viewMode === 'json' || !changedCount" title="全部恢复原值">
          <template #icon><UndoOutlined /></template>全部恢复
        </a-button>
        <a-button size="small" :disabled="!changedCount" @click="openPreview" title="预览改写影响（dry-run）">
          <template #icon><EyeOutlined /></template>预览
        </a-button>
        <a-button size="small" @click="saveFile" :disabled="!dirty">
          <template #icon><SaveOutlined /></template>保存
        </a-button>
      </div>
    </div>

    <!-- path bar -->
    <div v-if="filePath" class="rde-path-bar" :title="filePath">
      <span class="rde-path-label">文件:</span>
      <code class="rde-path-val">{{ filePath }}</code>
    </div>

    <!-- ============ 表单模式 ============ -->
    <template v-if="viewMode === 'table'">
      <!-- empty state -->
      <div v-if="!hasData && !loading" class="rde-empty">
        <div class="rde-empty-card">
          <TableOutlined class="rde-empty-icon" />
          <div class="rde-empty-title">暂无导出数据</div>
          <div class="rde-empty-desc">请先在「改写」页导出提交数据，<br/>导出后会自动生成可编辑的数据文件</div>
        </div>
      </div>

      <!-- table -->
      <div v-else class="rde-table-wrap">
        <a-table
          :columns="columns"
          :data-source="rows"
          row-key="hash"
          size="small"
          :pagination="false"
          :expandable="{ expandedRowKeys, onExpandedRowsChange: (keys) => expandedRowKeys = keys }"
          :loading="loading"
        >
          <template #bodyCell="{ column, record, index }">
            <template v-if="column.key === 'idx'">
              <span class="rde-idx">{{ rowIndexOf.get(record.hash) }}</span>
            </template>
            <template v-else-if="column.key === 'status'">
              <span class="rde-dot" :class="{ changed: isChanged(record) }" :title="isChanged(record) ? '已改 ' + changedFieldNames(record).length + ' 项' : '未改'">
                {{ isChanged(record) ? '●' : '·' }}
              </span>
            </template>
            <template v-else-if="column.key === 'hash'">
              <span class="rde-hash">{{ record.hash.slice(0, 7) }}</span>
            </template>
            <template v-else-if="column.key === 'authorName'">
              <span :class="{ 'rde-cell-changed': fieldChanged(record, 'authorName') }">{{ record.rewrite.authorName || '—' }}</span>
            </template>
            <template v-else-if="column.key === 'authorEmail'">
              <span :class="{ 'rde-cell-changed': fieldChanged(record, 'authorEmail') }">{{ record.rewrite.authorEmail || '—' }}</span>
            </template>
            <template v-else-if="column.key === 'authorDate'">
              <span :class="{ 'rde-cell-changed': fieldChanged(record, 'authorDate') }">{{ record.rewrite.authorDate || '—' }}</span>
            </template>
            <template v-else-if="column.key === 'subject'">
              <span :class="{ 'rde-cell-changed': fieldChanged(record, 'message') }">{{ (record.rewrite.message || '').split('\n')[0] || '—' }}</span>
            </template>
            <template v-else-if="column.key === 'pushed'">
              <span class="rde-push" :class="record.pushed ? 'is-pushed' : 'is-local'">
                {{ record.pushed ? '已推送' : '本地' }}
              </span>
            </template>
          </template>

          <!-- expanded row: inline editor -->
          <template #expandedRowRender="{ record }">
            <RewriteRowEditor
              v-if="getRow(record.hash)"
              :row="getRow(record.hash)"
              :ai-loading="aiLoadingHash === record.hash"
              @dirty="markDirty"
              @restore="restoreRow"
              @ai-message="aiMessage"
            />
          </template>
        </a-table>
      </div>
    </template>

    <!-- ============ JSON 模式 ============ -->
    <div v-else class="rde-json-wrap">
      <div v-if="!hasData && !loading" class="rde-empty">
        <div class="rde-empty-card">
          <CodeOutlined class="rde-empty-icon" />
          <div class="rde-empty-title">暂无数据</div>
          <div class="rde-empty-desc">请先在「改写」页导出提交数据</div>
        </div>
      </div>
      <CodeViewer
        v-else
        :code="jsonText"
        filename="data.json"
        editable
        @update:modelValue="onJsonEdit"
      />
    </div>

    <!-- preview modal (dry-run) -->
    <RewritePreviewModal
      v-model:open="previewVisible"
      :rows="rows"
      :repo-arg="repo.repoArg()"
      @applied="loadFromDisk"
    />
  </div>
</template>

<style scoped>
.rde-view {
  width: calc(100% + var(--spacing-lg) * 2);
  height: calc(100% + var(--spacing-lg) * 2);
  margin: calc(-1 * var(--spacing-lg));
  display: flex;
  flex-direction: column;
  background: transparent;
  min-height: 0;
}

/* toolbar — flush strip, aligned with other pages' status bars */
.rde-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 38px;
  padding: 0 var(--spacing-md);
  border-bottom: 1px solid var(--color-border);
  flex-shrink: 0;
  flex-wrap: wrap;
  gap: 8px;
}
.rde-tb-left { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; min-width: 0; }
.rde-tb-right { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; flex-shrink: 0; }
.rde-title { font-size: 12px; font-weight: 600; color: var(--color-text-secondary); }
.rde-stat { font-size: 12px; color: var(--color-text-secondary); }
.rde-stat b { color: var(--color-text); }
.rde-changed { color: var(--color-primary); }
.rde-dirty { font-size: 11px; color: var(--color-warning); font-weight: 600; }

/* path bar — flush strip directly under the toolbar */
.rde-path-bar {
  display: flex;
  align-items: center;
  gap: 6px;
  height: 28px;
  padding: 0 var(--spacing-md);
  background: var(--color-background);
  border-bottom: 1px solid var(--color-border);
  flex-shrink: 0;
}
.rde-path-label { font-size: 11px; color: var(--color-text-tertiary); flex-shrink: 0; }
.rde-path-val {
  font-family: 'Cascadia Code', monospace;
  font-size: 11px;
  color: var(--color-text-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* table */
.rde-table-wrap { flex: 1; min-height: 0; overflow: auto; padding: 0 16px 16px; }
.rde-idx { font-family: 'Cascadia Code', monospace; font-size: 11px; color: var(--color-text-tertiary); }
.rde-dot { font-size: 12px; color: var(--color-text-tertiary); }
.rde-dot.changed { color: var(--color-primary); }
.rde-hash { font-family: 'Cascadia Code', monospace; font-size: 11px; color: var(--color-hash); }
.rde-cell-changed { color: var(--color-primary); font-weight: 500; }
.rde-push { font-size: 10px; padding: 1px 6px; border-radius: 8px; }
.rde-push.is-pushed { color: var(--color-text-tertiary); background: var(--color-surface-raised); }
.rde-push.is-local { color: var(--color-success); background: var(--color-success-bg); }

/* json mode */
.rde-json-wrap { flex: 1; min-height: 0; overflow: hidden; padding: 0 16px 16px; }

/* empty */
.rde-empty { flex: 1; display: flex; align-items: center; justify-content: center; }
.rde-empty-card { display: flex; flex-direction: column; align-items: center; gap: 10px; padding: 48px 40px; text-align: center; max-width: 400px; }
.rde-empty-icon { font-size: 40px; color: var(--color-primary); opacity: 0.6; }
.rde-empty-title { font-size: 16px; font-weight: 600; color: var(--color-text); }
.rde-empty-desc { font-size: 13px; color: var(--color-text-secondary); line-height: 1.7; }
</style>
