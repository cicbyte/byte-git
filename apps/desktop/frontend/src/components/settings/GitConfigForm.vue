<script setup>
import { ref, computed, watch, onMounted } from 'vue'
import { useGitConfig } from '../../composables/useGitConfig'

const { entries, loading, load, queueSave } = useGitConfig()

// per-entry editable state: { [key]: { scope, value } }
const edits = ref({})

function syncEdits() {
  const next = {}
  entries.value.forEach((e) => {
    // default scope: keep existing layer if set, else global
    const scope = e.hasLocal ? 'local' : 'global'
    next[e.key] = { scope, value: e.effectiveValue }
  })
  edits.value = next
}

onMounted(load)
watch(entries, syncEdits, { immediate: true })

// sections
const sections = [
  { title: '身份', keys: ['user.name', 'user.email'] },
  { title: '编辑器 / 换行', keys: ['core.editor', 'core.autocrlf', 'core.quotepath'] },
  { title: '行为', keys: ['pull.rebase', 'init.defaultBranch', 'commit.gpgsign'] },
  { title: '网络', keys: ['http.proxy'] },
]

const keyLabel = {
  'user.name': '用户名',
  'user.email': '邮箱',
  'core.editor': '默认编辑器',
  'core.autocrlf': '换行符处理',
  'core.quotepath': '中文路径',
  'pull.rebase': '拉取时变基',
  'init.defaultBranch': '默认分支',
  'commit.gpgsign': '签名提交',
  'http.proxy': 'HTTP 代理',
}

// options for enum-typed keys
const enumOptions = {
  'core.autocrlf': [
    { label: 'true (签入转 CRLF, 签出转 LF)', value: 'true' },
    { label: 'input (仅签出转 LF)', value: 'input' },
    { label: 'false (不转换)', value: 'false' },
  ],
  'pull.rebase': [
    { label: 'false (merge)', value: 'false' },
    { label: 'true (rebase)', value: 'true' },
    { label: 'merges', value: 'merges' },
  ],
  'commit.gpgsign': [
    { label: '是', value: 'true' },
    { label: '否', value: 'false' },
  ],
}

function findEntry(key) {
  return entries.value.find((e) => e.key === key)
}
function isEnum(key) {
  return !!enumOptions[key]
}
function sourceTag(e) {
  if (!e) return ''
  if (e.hasLocal && e.hasGlobal) return 'local 覆盖 global'
  if (e.hasLocal) return 'local'
  if (e.hasGlobal) return 'global'
  return '默认'
}
function isOverridden(e) {
  return e && e.hasLocal && e.hasGlobal
}

// input change handlers
function onValueChange(key, v) {
  if (!edits.value[key]) return
  edits.value[key].value = v
  queueSave(key, edits.value[key].scope, v)
}
function onScopeChange(key, v) {
  if (!edits.value[key]) return
  edits.value[key].scope = v
  // re-save current value at the new scope
  queueSave(key, v, edits.value[key].value)
}
</script>

<template>
  <div class="settings-form">
    <a-spin :spinning="loading">
      <div v-for="s in sections" :key="s.title" class="setting-group">
        <div class="group-title">{{ s.title }}</div>

        <div v-for="key in s.keys" :key="key" class="gc-row">
          <div class="gc-label">
            <div class="gc-name">{{ keyLabel[key] || key }}</div>
            <div class="gc-key">{{ key }}</div>
          </div>

          <div class="gc-control">
            <a-select
              v-if="isEnum(key)"
              :value="edits[key]?.value"
              :options="enumOptions[key]"
              size="small"
              style="width: 260px"
              @change="(v) => onValueChange(key, v)"
            />
            <a-input
              v-else
              :value="edits[key]?.value"
              :placeholder="`(未设置)`"
              size="small"
              style="width: 260px"
              @change="(e) => onValueChange(key, e.target.value)"
            />
          </div>

          <div class="gc-scope">
            <span class="src-tag" :class="{ override: isOverridden(findEntry(key)) }">
              {{ sourceTag(findEntry(key)) }}
            </span>
            <a-select
              :value="edits[key]?.scope || 'global'"
              size="small"
              style="width: 100px"
              @change="(v) => onScopeChange(key, v)"
            >
              <a-select-option value="global">全局</a-select-option>
              <a-select-option value="local">仓库</a-select-option>
            </a-select>
          </div>
        </div>
      </div>
    </a-spin>
  </div>
</template>

<style scoped>
.settings-form {
  flex: 1;
  overflow-y: auto;
  padding: 20px 24px;
  background: var(--color-background);
}

.setting-group {
  margin-bottom: var(--spacing-md);
  border-radius: var(--radius-md);
  border: 1px solid var(--color-border);
  padding: 16px 18px;
  background: var(--color-surface);
  box-shadow: var(--shadow-xs);
}

.group-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--color-text);
  margin-bottom: 12px;
}

.gc-row {
  display: flex;
  align-items: center;
  gap: var(--spacing-sm);
  min-height: 40px;
  padding: 4px 0;
}

.gc-label {
  width: 140px;
  flex-shrink: 0;
}

.gc-name {
  font-size: 13px;
  color: var(--color-text);
}

.gc-key {
  font-size: 11px;
  color: var(--color-text-tertiary);
  font-family: 'Cascadia Code', monospace;
}

.gc-control {
  flex-shrink: 0;
}

.gc-scope {
  margin-left: auto;
  display: flex;
  align-items: center;
  gap: 6px;
}

.src-tag {
  font-size: 11px;
  color: var(--color-text-tertiary);
  padding: 2px 8px;
  border-radius: 8px;
  background: var(--color-surface-raised);
}

.src-tag.override {
  color: var(--color-warning);
  background: var(--color-warning-bg);
}
</style>
