<script setup>
/** 健康检查 — reference doctor。展示检查项列表 + 状态汇总。 */
import { ref, computed } from 'vue'
import { message } from 'ant-design-vue'
import { ReloadOutlined, CheckCircleOutlined, WarningOutlined, CloseCircleOutlined } from '@ant-design/icons-vue'

const app = window.go?.main?.ByteGitApp
const doctor = ref(null)
const loading = ref(false)

async function run() {
  loading.value = true
  doctor.value = null
  try {
    doctor.value = await app.ReferenceDoctor()
  } catch (e) {
    message.error('健康检查失败: ' + String(e?.message || e))
  } finally {
    loading.value = false
  }
}

const summary = computed(() => {
  if (!doctor.value?.checks?.length) return null
  const has = (s) => doctor.value.checks.some((c) => c.status === s)
  if (has('error')) return { icon: CloseCircleOutlined, cls: 'err', text: '存在错误' }
  if (has('warn')) return { icon: WarningOutlined, cls: 'warn', text: '存在警告' }
  return { icon: CheckCircleOutlined, cls: 'ok', text: '全部正常' }
})

function statusColor(s) {
  return s === 'ok' ? 'var(--color-success)' : s === 'warn' ? 'var(--color-warning)' : 'var(--color-error)'
}
</script>

<template>
  <div class="section">
    <div class="sec-card">
      <div class="card-head">
        <div class="card-title">健康检查</div>
        <div class="card-desc">诊断当前项目的引用健康状态:软链接完整性、Wiki Junction、Reference Map、数据库一致性、Wiki Git、Agent 配置。</div>
      </div>
      <div class="card-body">
        <div class="card-toolbar" style="margin-bottom: 14px">
          <a-button type="primary" :loading="loading" @click="run">
            <template #icon><ReloadOutlined /></template>运行检查
          </a-button>
          <span v-if="summary" :class="summary.cls" style="font-weight: 600">
            <component :is="summary.icon" /> {{ summary.text }}
          </span>
        </div>

        <div v-if="loading" style="padding: 30px; text-align: center"><a-spin /></div>
        <div v-else-if="doctor">
          <div
            v-for="(c, i) in doctor.checks" :key="i"
            class="list-row"
            style="grid-template-columns: 14px 1fr 1fr 80px;"
          >
            <span :style="{ color: statusColor(c.status), fontSize: '10px' }">●</span>
            <span style="color: var(--color-text); font-weight: 500">{{ c.name }}</span>
            <span style="color: var(--color-text-secondary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">{{ c.details }}</span>
            <span style="color: var(--color-text-tertiary); font-size: 10px; text-align: right;">{{ c.group }}</span>
          </div>
        </div>
        <div v-else class="hint" style="text-align: center; padding: 30px">点击「运行检查」开始诊断</div>
      </div>
    </div>
  </div>
</template>

<style scoped>
@import './section.css';
.ok { color: var(--color-success); }
.warn { color: var(--color-warning); }
.err { color: var(--color-error); }
</style>
