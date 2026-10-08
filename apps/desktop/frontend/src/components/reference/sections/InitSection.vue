<script setup>
/** 初始化 — reference init [--agent X]。幂等,可重复执行。
 * 加载时展示当前项目的初始化状态(是否已初始化 + 已注入的 agents),
 * 数据来自 .reference/reference.settings.json(经 ReferenceSettings 绑定)。 */
import { ref, computed, onMounted } from 'vue'
import { message } from 'ant-design-vue'
import {
  ThunderboltOutlined, CheckCircleOutlined, CloseCircleOutlined,
} from '@ant-design/icons-vue'

const app = window.go?.main?.ByteGitApp
const agent = ref('none')
const loading = ref(false)
const settings = ref(null) // ReferenceSettings | null (null = loading)
const agentOptions = [
  { value: 'none', label: '不注入' },
  { value: 'claude', label: 'Claude' },
  { value: 'codex', label: 'Codex' },
  { value: 'opencode', label: 'OpenCode' },
  { value: 'zcode', label: 'ZCode' },
  { value: 'mimocode', label: 'MimoCode' },
]

async function loadSettings() {
  if (!app?.ReferenceSettings) return
  try {
    settings.value = await app.ReferenceSettings()
  } catch {
    settings.value = { hasFile: false, initialized: false, agents: [] }
  }
}

async function submit() {
  loading.value = true
  try {
    await app.ReferenceInit({ agent: agent.value })
    message.success(`已初始化(当前项目, agent=${agent.value})`)
    await loadSettings() // refresh status
  } catch (e) {
    message.error('初始化失败: ' + String(e?.message || e))
  } finally {
    loading.value = false
  }
}

const initialized = computed(() => !!settings.value?.initialized)
const hasFile = computed(() => !!settings.value?.hasFile)
const currentAgents = computed(() => settings.value?.agents || [])

onMounted(loadSettings)
</script>

<template>
  <div class="section">
    <div class="sec-card">
      <div class="card-head">
        <div class="card-title">项目初始化</div>
        <div class="card-desc">在当前项目下创建 <code>.reference/</code> 目录结构、生成 settings/map、更新 .gitignore,并可选注入 AI Agent 配置。</div>
      </div>
      <div class="card-body">
        <!-- current status -->
        <div class="status-row">
          <template v-if="settings && hasFile">
            <CheckCircleOutlined v-if="initialized" class="ok" />
            <CloseCircleOutlined v-else class="err" />
            <span v-if="initialized" class="ok" style="font-weight: 600">已初始化</span>
            <span v-else class="err" style="font-weight: 600">未初始化</span>
            <template v-if="currentAgents.length">
              <span style="color: var(--color-text-tertiary);">已注入 Agent:</span>
              <a-tag v-for="a in currentAgents" :key="a" color="purple">{{ a }}</a-tag>
            </template>
            <span v-else-if="initialized" class="hint">无 Agent 注入</span>
          </template>
          <template v-else-if="settings && !hasFile">
            <CloseCircleOutlined class="err" />
            <span class="err" style="font-weight: 600">未初始化</span>
            <span class="hint">.reference/ 目录不存在,执行下方初始化以创建</span>
          </template>
          <template v-else>
            <a-spin size="small" />
            <span class="hint">读取中...</span>
          </template>
        </div>

        <!-- init form -->
        <div class="form-row">
          <label>AI Agent</label>
          <a-select v-model:value="agent" :options="agentOptions" style="width: 200px" />
          <a-button type="primary" :loading="loading" @click="submit">
            <template #icon><ThunderboltOutlined /></template>执行初始化
          </a-button>
        </div>

        <a-alert type="info" show-icon style="margin-top: 16px">
          <template #message>幂等操作</template>
          <template #description>重复执行安全。已存在的 <code>.reference/</code> 与配置会保留,仅补全缺失项。重新选择 Agent 会更新注入的 AI 配置文件。</template>
        </a-alert>
      </div>
    </div>
  </div>
</template>

<style scoped>
@import './section.css';
.ok { color: var(--color-success); }
.err { color: var(--color-error); }
</style>
