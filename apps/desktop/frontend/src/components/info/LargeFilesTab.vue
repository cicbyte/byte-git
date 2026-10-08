<script setup>
import { ref, onMounted } from 'vue'
import { message as AMessage } from 'ant-design-vue'
import { WarningOutlined, FireOutlined, ReloadOutlined } from '@ant-design/icons-vue'
import { useRepoStore } from '../../stores/repo'
import PanelState from '../common/PanelState.vue'

const repo = useRepoStore()
const app = window.go?.main?.ByteGitApp

const data = ref(null)
const loading = ref(false)
const error = ref('')

async function reload() {
  if (!app?.InfoLargeFiles) return
  loading.value = true
  error.value = ''
  try {
    data.value = await app.InfoLargeFiles(repo.repoArg())
  } catch (e) {
    error.value = String(e?.message || e)
  } finally {
    loading.value = false
  }
}

onMounted(reload)

function rowClass(f) {
  if (f.lines >= 500) return 'row-critical'
  if (f.lines >= 300) return 'row-warning'
  return ''
}

function fmtSize(bytes) {
  if (!bytes) return '0 B'
  if (bytes < 1024) return bytes + ' B'
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB'
  return (bytes / 1024 / 1024).toFixed(1) + ' MB'
}
</script>

<template>
  <div class="lf-wrap">
    <PanelState :loading="loading" :error="error" :empty="!data?.files?.length" empty-text="无大文件" @retry="reload">
    <!-- summary bar -->
    <div v-if="data" class="lf-summary">
      <span class="lf-stat warn" v-if="data.warning">
        <WarningOutlined /> {{ data.warning }} 个警告 (≥300行)
      </span>
      <span class="lf-stat crit" v-if="data.critical">
        <FireOutlined /> {{ data.critical }} 个严重 (≥500行)
      </span>
      <span class="lf-stat muted">Top {{ data.files.length }} 文件</span>
      <a-button size="small" type="text" :loading="loading" @click="reload" title="刷新">
        <template #icon><ReloadOutlined /></template>
      </a-button>
    </div>

    <!-- table -->
    <table v-if="data?.files?.length" class="lf-table">
      <thead>
        <tr>
          <th class="lf-th-rank">#</th>
          <th class="lf-th-path">文件</th>
          <th class="lf-th-lang">语言</th>
          <th class="lf-th-lines">行数</th>
          <th class="lf-th-size">大小</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="(f, i) in data.files" :key="f.path" :class="rowClass(f)">
          <td class="lf-td-rank">{{ i + 1 }}</td>
          <td class="lf-td-path" :title="f.path">{{ f.path }}</td>
          <td class="lf-td-lang"><span class="lf-lang-tag">{{ f.language }}</span></td>
          <td class="lf-td-lines">
            <span class="lf-lines-val">{{ f.lines }}</span>
            <div class="lf-lines-bar">
              <div class="lf-lines-fill" :class="rowClass(f)" :style="{ width: Math.min(100, f.lines / 10) + '%' }"></div>
            </div>
          </td>
          <td class="lf-td-size">{{ fmtSize(f.size) }}</td>
        </tr>
      </tbody>
    </table>
    </PanelState>
  </div>
</template>

<style scoped>
.lf-wrap { display: flex; flex-direction: column; height: 100%; }
.lf-summary { display: flex; align-items: center; gap: 16px; margin-bottom: 12px; }
.lf-stat { font-size: 12px; display: inline-flex; align-items: center; gap: 4px; }
.lf-stat.warn { color: var(--color-warning); font-weight: 600; }
.lf-stat.crit { color: var(--color-error); font-weight: 600; }
.lf-stat.muted { color: var(--color-text-tertiary); }

.lf-table { width: 100%; border-collapse: collapse; }
.lf-th-rank, .lf-td-rank { width: 36px; text-align: center; font-family: 'Cascadia Code', monospace; }
.lf-th-path, .lf-td-path { text-align: left; }
.lf-th-lang, .lf-td-lang { width: 90px; text-align: center; }
.lf-th-lines, .lf-td-lines { width: 200px; }
.lf-th-size, .lf-td-size { width: 80px; text-align: right; font-family: 'Cascadia Code', monospace; }

.lf-th-rank, .lf-th-path, .lf-th-lang, .lf-th-lines, .lf-th-size {
  font-size: 11px; font-weight: 600; color: var(--color-text-tertiary);
  padding: 6px 8px; text-transform: uppercase; border-bottom: 1px solid var(--color-border);
}

.lf-td-rank, .lf-td-path, .lf-td-lang, .lf-td-lines, .lf-td-size {
  padding: 5px 8px; font-size: 12px; border-bottom: 1px solid var(--color-border-light);
}

.lf-td-path { font-family: 'Cascadia Code', monospace; color: var(--color-text); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 0; }
.lf-lang-tag { font-size: 10px; padding: 1px 6px; border-radius: 3px; background: var(--color-info-bg); color: var(--color-info); font-weight: 500; }

.lf-td-lines { display: flex; align-items: center; gap: 8px; }
.lf-lines-val { font-family: 'Cascadia Code', monospace; font-weight: 600; min-width: 50px; flex-shrink: 0; }
.lf-lines-bar { flex: 1; height: 4px; background: var(--color-surface-raised); border-radius: 2px; overflow: hidden; }
.lf-lines-fill { height: 100%; border-radius: 2px; transition: width var(--transition-normal); }
.lf-lines-fill.row-warning { background: var(--color-warning); }
.lf-lines-fill.row-critical { background: var(--color-error); }
.lf-lines-fill:not(.row-warning):not(.row-critical) { background: var(--color-primary); }

tr.row-warning .lf-lines-val { color: var(--color-warning); }
tr.row-critical .lf-lines-val { color: var(--color-error); }
tr.row-warning { background: rgba(255, 184, 108, 0.04); }
tr.row-critical { background: rgba(255, 85, 85, 0.04); }
</style>
