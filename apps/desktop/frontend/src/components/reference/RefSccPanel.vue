<script setup>
/**
 * 代码统计面板 — 展示 reference repo scc 命令的结果：
 *   - 语言分布（条形图 + 表格）
 *   - Top 文件排名（按代码量）
 */
import { ref, watch, computed } from 'vue'
import { BarChartOutlined, FireOutlined } from '@ant-design/icons-vue'

const props = defineProps({
  repoName: { type: String, default: '' },
})

const app = window.go?.main?.ByteGitApp

const data = ref(null)
const loading = ref(false)
const error = ref('')

async function load() {
  if (!repoName.value || !app?.ReferenceScc) return
  loading.value = true
  error.value = ''
  data.value = null
  try {
    data.value = await app.ReferenceScc(props.repoName)
  } catch (e) {
    error.value = String(e?.message || e)
  } finally {
    loading.value = false
  }
}

const repoName = computed(() => props.repoName)
watch(() => props.repoName, load, { immediate: true })

const totalCode = computed(() =>
  (data.value?.languages || []).reduce((s, l) => s + (l.code || 0), 0),
)

// top 6 languages for the bar chart; rest bucketed into "其他"
const chartLangs = computed(() => {
  if (!data.value?.languages?.length) return []
  const sorted = [...data.value.languages].sort((a, b) => b.code - a.code)
  const top = sorted.slice(0, 6)
  const rest = sorted.slice(6)
  if (rest.length) {
    const restSum = rest.reduce((s, l) => s + (l.code || 0), 0)
    top.push({ Language: '其他', Code: restSum, _rest: true })
  }
  return top.map((l) => ({
    name: l.Language || l.languages || '未知',
    code: l.code ?? l.Code ?? 0,
    pct: totalCode.value ? (((l.code ?? l.Code ?? 0) / totalCode.value) * 100) : 0,
  }))
})

// distinct colors for the bar chart (cycle through a fixed palette)
const palette = ['#7d6cf8', '#ff79c6', '#8be9fd', '#50fa7b', '#f1fa8c', '#ffb86c', '#bd93f9', '#6272a4']
function colorOf(i) { return palette[i % palette.length] }

function fmt(n) {
  if (n >= 1e6) return (n / 1e6).toFixed(1) + 'M'
  if (n >= 1e3) return (n / 1e3).toFixed(1) + 'K'
  return String(n || 0)
}
</script>

<template>
  <div class="scc-panel">
    <div v-if="loading" class="sp-loading"><a-spin tip="统计代码中..." /></div>
    <div v-else-if="error" class="sp-error">{{ error }}</div>
    <div v-else-if="!data" class="sp-empty">暂无数据</div>
    <div v-else class="sp-body">
      <!-- summary -->
      <div class="sp-summary">
        <div class="ss-card">
          <div class="ss-val">{{ fmt(totalCode) }}</div>
          <div class="ss-lbl">总代码行</div>
        </div>
        <div class="ss-card">
          <div class="ss-val">{{ data.languages.length }}</div>
          <div class="ss-lbl">语言数</div>
        </div>
        <div class="ss-card">
          <div class="ss-val">{{ fmt(data.topFiles.reduce((s, f) => s + (f.code || 0), 0)) }}</div>
          <div class="ss-lbl">Top {{ data.topFiles.length }} 文件合计</div>
        </div>
      </div>

      <!-- language distribution -->
      <div class="sp-section">
        <div class="sec-head"><BarChartOutlined class="sh-icon" /><span>语言分布</span></div>
        <div class="bar-chart">
          <div v-for="(l, i) in chartLangs" :key="l.name" class="bar-row">
            <span class="bar-name">{{ l.name }}</span>
            <div class="bar-track">
              <div class="bar-fill" :style="{ width: l.pct + '%', background: colorOf(i) }"></div>
            </div>
            <span class="bar-code">{{ fmt(l.code) }}</span>
            <span class="bar-pct">{{ l.pct.toFixed(1) }}%</span>
          </div>
        </div>
      </div>

      <!-- top files -->
      <div class="sp-section">
        <div class="sec-head"><FireOutlined class="sh-icon" /><span>Top 文件（按代码行）</span></div>
        <div class="file-table">
          <div class="ft-head">
            <span class="ftc-rank">#</span>
            <span class="ftc-file">文件</span>
            <span class="ftc-lang">语言</span>
            <span class="ftc-code">代码行</span>
            <span class="ftc-cplx">复杂度</span>
          </div>
          <div v-for="(f, i) in data.topFiles" :key="f.location" class="ft-row">
            <span class="ftc-rank" :class="{ top: i < 3 }">{{ i + 1 }}</span>
            <span class="ftc-file" :title="f.location">{{ f.filename }}</span>
            <span class="ftc-lang">{{ f.language }}</span>
            <span class="ftc-code">{{ fmt(f.code) }}</span>
            <span class="ftc-cplx">{{ f.complexity || '—' }}</span>
          </div>
          <div v-if="!data.topFiles.length" class="ft-empty">无文件数据</div>
        </div>
      </div>

      <!-- full language table -->
      <details class="sp-section collapsible">
        <summary class="sec-head"><span>全部语言明细</span></summary>
        <div class="lang-table">
          <div class="lt-head">
            <span>语言</span><span>文件</span><span>代码</span><span>注释</span><span>复杂度</span>
          </div>
          <div v-for="l in data.languages" :key="l.Language" class="lt-row">
            <span>{{ l.Language || l.languages }}</span>
            <span>{{ l.files }}</span>
            <span>{{ fmt(l.code) }}</span>
            <span>{{ fmt(l.comments) }}</span>
            <span>{{ l.complexity || '—' }}</span>
          </div>
        </div>
      </details>
    </div>
  </div>
</template>

<style scoped>
.scc-panel { height: 100%; overflow-y: auto; padding: 16px 18px; }
.sp-loading, .sp-empty, .sp-error {
  height: 100%; display: flex; align-items: center; justify-content: center;
  color: var(--color-text-tertiary); font-size: 13px;
}
.sp-error { color: var(--color-error); }

/* summary cards */
.sp-summary { display: grid; grid-template-columns: repeat(3, 1fr); gap: 12px; margin-bottom: 20px; }
.ss-card {
  background: var(--color-surface); border: 1px solid var(--color-border);
  border-radius: var(--radius-md); padding: 14px 16px; box-shadow: var(--shadow-xs);
}
.ss-val { font-size: 22px; font-weight: 700; color: var(--color-text); line-height: 1.2; }
.ss-lbl { font-size: 11px; color: var(--color-text-tertiary); margin-top: 4px; }

/* section */
.sp-section { margin-bottom: 18px; }
.sec-head {
  display: flex; align-items: center; gap: 8px;
  font-size: 13px; font-weight: 600; color: var(--color-text);
  margin-bottom: 10px;
}
.sh-icon { color: var(--color-primary); }

/* bar chart */
.bar-chart { display: flex; flex-direction: column; gap: 6px; }
.bar-row { display: grid; grid-template-columns: 90px 1fr 60px 48px; gap: 10px; align-items: center; font-size: 12px; }
.bar-name { color: var(--color-text-secondary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.bar-track { height: 10px; background: var(--color-background); border-radius: 5px; overflow: hidden; }
.bar-fill { height: 100%; border-radius: 5px; transition: width var(--transition-normal); }
.bar-code { font-family: 'Cascadia Code', monospace; color: var(--color-text); text-align: right; }
.bar-pct { color: var(--color-text-tertiary); text-align: right; }

/* file table */
.file-table { border: 1px solid var(--color-border-light); border-radius: var(--radius-sm); overflow: hidden; }
.ft-head, .ft-row {
  display: grid; grid-template-columns: 36px 1fr 110px 80px 70px;
  gap: 8px; padding: 7px 12px; align-items: center; font-size: 12px;
}
.ft-head {
  background: var(--color-surface); color: var(--color-text-tertiary);
  font-size: 11px; font-weight: 600; border-bottom: 1px solid var(--color-border-light);
}
.ft-row { border-bottom: 1px solid var(--color-border-light); }
.ft-row:last-child { border-bottom: none; }
.ft-row:hover { background: var(--color-hover); }
.ftc-rank { color: var(--color-text-tertiary); }
.ftc-rank.top { color: var(--color-accent); font-weight: 700; }
.ftc-file { color: var(--color-text); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-family: 'Cascadia Code', monospace; }
.ftc-lang { color: var(--color-text-secondary); }
.ftc-code { font-family: 'Cascadia Code', monospace; color: var(--color-text); text-align: right; }
.ftc-cplx { font-family: 'Cascadia Code', monospace; color: var(--color-text-tertiary); text-align: right; }
.ft-empty { padding: 16px; text-align: center; font-size: 12px; color: var(--color-text-tertiary); }

/* collapsible lang table */
.collapsible summary { cursor: pointer; list-style: none; user-select: none; }
.collapsible summary::-webkit-details-marker { display: none; }
.lang-table { border: 1px solid var(--color-border-light); border-radius: var(--radius-sm); overflow: hidden; margin-top: 8px; }
.lt-head, .lt-row {
  display: grid; grid-template-columns: 1.4fr 0.6fr 0.8fr 0.8fr 0.8fr;
  gap: 8px; padding: 6px 12px; font-size: 12px;
}
.lt-head { background: var(--color-surface); color: var(--color-text-tertiary); font-size: 11px; border-bottom: 1px solid var(--color-border-light); }
.lt-row { border-bottom: 1px solid var(--color-border-light); color: var(--color-text-secondary); }
.lt-row:last-child { border-bottom: none; }
</style>
