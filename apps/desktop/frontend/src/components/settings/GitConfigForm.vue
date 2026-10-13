<script setup>
import { ref, computed, watch, onMounted } from 'vue'
import { useGitConfig } from '../../composables/useGitConfig'

const { entries, loading, load, queueSave } = useGitConfig()

// active tab: 'global' | 'local'
const activeScope = ref('global')

// per-entry editable state: { [key]: value }
const edits = ref({})

function syncEdits() {
  const next = {}
  entries.value.forEach((e) => {
    next[e.key] = e.effectiveValue
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
  { title: 'Hook', keys: ['core.hooksPath'] },
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
  'core.hooksPath': 'Hooks 目录',
  'http.proxy': 'HTTP 代理',
}

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

// value for the current tab scope
function scopeValue(key) {
  const e = findEntry(key)
  if (!e) return ''
  return activeScope.value === 'local' ? e.localValue : e.globalValue
}
function hasScopeValue(key) {
  const e = findEntry(key)
  if (!e) return false
  return activeScope.value === 'local' ? e.hasLocal : e.hasGlobal
}
// overridden = both local and global set (local shadows global)
function isOverridden(key) {
  const e = findEntry(key)
  return e && e.hasLocal && e.hasGlobal
}

function onValueChange(key, v) {
  edits.value[key] = v
  // write at the current tab's scope
  queueSave(key, activeScope.value, v)
}
</script>

<template>
  <div class="settings-form">
    <a-spin :spinning="loading">
      <!-- scope tabs -->
      <div class="gc-scope-tabs">
        <a-radio-group v-model:value="activeScope" button-style="solid" size="small">
          <a-radio-button value="global">全局</a-radio-button>
          <a-radio-button value="local">仓库级</a-radio-button>
        </a-radio-group>
        <span v-if="activeScope === 'local'" class="gc-scope-hint">
          仓库级设置覆盖全局，仅影响当前仓库
        </span>
        <span v-else class="gc-scope-hint">
          全局设置对所有仓库生效
        </span>
      </div>

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
              :value="scopeValue(key)"
              :options="enumOptions[key]"
              size="small"
              style="width: 260px"
              :placeholder="hasScopeValue(key) ? '' : '(未设置)'"
              @change="(v) => onValueChange(key, v)"
            />
            <a-input
              v-else
              :value="scopeValue(key)"
              :placeholder="hasScopeValue(key) ? '' : '(未设置)'"
              size="small"
              style="width: 260px"
              @change="(e) => onValueChange(key, e.target.value)"
            />
          </div>

          <div class="gc-status">
            <span v-if="isOverridden(key) && activeScope === 'global'" class="src-tag override" title="仓库级覆盖了此项">
              被覆盖
            </span>
            <span v-else-if="hasScopeValue(key)" class="src-tag">已设置</span>
            <span v-else class="src-tag muted">未设置</span>
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

.gc-scope-tabs {
  display: flex;
  align-items: center;
  gap: var(--spacing-sm);
  margin-bottom: var(--spacing-md);
}
.gc-scope-hint {
  font-size: 11px;
  color: var(--color-text-tertiary);
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

.gc-status {
  margin-left: auto;
}

.src-tag {
  font-size: 11px;
  color: var(--color-text-tertiary);
  padding: 2px 8px;
  border-radius: 8px;
  background: var(--color-surface-raised);
}
.src-tag.muted { opacity: 0.5; }
.src-tag.override {
  color: var(--color-warning);
  background: var(--color-warning-bg);
}
</style>
