<script setup>
/**
 * GitHub .gitignore template picker modal.
 *
 * Visibility is controlled via v-model:open. Owns the lightweight UI state
 * (search query + selection), and emits `apply` with the chosen template names
 * for the parent to preview/write.
 */
import { ref, computed } from 'vue'
import { CheckCircleOutlined, CloudSyncOutlined } from '@ant-design/icons-vue'

const props = defineProps({
  open: { type: Boolean, default: false },
  templates: { type: Array, default: () => [] },
  githubCloned: { type: Boolean, default: false },
  syncLoading: { type: Boolean, default: false },
  addLoading: { type: Boolean, default: false },
})

const emit = defineEmits(['update:open', 'syncGithub', 'apply'])

const search = ref('')
const selected = ref([])

const filtered = computed(() => {
  if (!search.value.trim()) return props.templates
  const q = search.value.toLowerCase()
  return props.templates.filter(t => t.toLowerCase().includes(q))
})

function togglePick(name) {
  const i = selected.value.indexOf(name)
  if (i >= 0) selected.value.splice(i, 1)
  else selected.value.push(name)
}

function close() { emit('update:open', false) }

function onApply() {
  if (!selected.value.length) return
  emit('apply', [...selected.value])
}

// reset selection when reopening
import { watch } from 'vue'
watch(() => props.open, (v) => {
  if (v) { search.value = ''; selected.value = [] }
})
</script>

<template>
  <a-modal :open="open" @update:open="emit('update:open', $event)" title="GitHub 模板" width="520px" :footer="null" destroy-on-close>
    <div class="gh-picker">
      <div class="gh-toolbar">
        <a-input v-model:value="search" placeholder="搜索模板名…" size="small" allow-clear class="gh-search" />
        <span class="gh-count">{{ filtered.length }} / {{ templates.length }}</span>
      </div>
      <div class="gh-not-synced" v-if="!githubCloned">
        <a-button type="primary" size="small" :loading="syncLoading" @click="emit('syncGithub')">
          <CloudSyncOutlined /> 同步模板库
        </a-button>
      </div>
      <div class="gh-list" v-else>
        <div v-for="t in filtered" :key="t" class="gh-item"
          :class="{ selected: selected.includes(t) }"
          @click="togglePick(t)">
          <span class="gh-name">{{ t }}</span>
          <CheckCircleOutlined v-if="selected.includes(t)" class="gh-check" />
        </div>
        <div v-if="!filtered.length" class="gh-empty">无匹配模板</div>
      </div>
      <div class="gh-actions" v-if="githubCloned">
        <span class="gh-sel-count" v-if="selected.length">已选 {{ selected.length }} 个</span>
        <div class="gh-spacer" />
        <a-button size="small" @click="close">取消</a-button>
        <a-button size="small" type="primary" :disabled="!selected.length" :loading="addLoading" @click="onApply">
          添加规则
        </a-button>
      </div>
    </div>
  </a-modal>
</template>

<style scoped>
.gh-picker { display: flex; flex-direction: column; gap: 10px; padding: 4px 0; }
.gh-toolbar { display: flex; align-items: center; gap: 8px; }
.gh-search { flex: 1; }
.gh-count { font-size: 11px; color: var(--color-text-tertiary); flex-shrink: 0; }
.gh-not-synced { display: flex; justify-content: center; padding: 16px 0; }
.gh-list { max-height: 320px; overflow-y: auto; border: 1px solid var(--color-border); border-radius: var(--radius-sm); }
.gh-item { display: flex; align-items: center; gap: 8px; padding: 6px 12px; cursor: pointer; font-size: 13px; }
.gh-item:hover { background: var(--color-hover); }
.gh-item.selected { background: var(--color-primary-bg); color: var(--color-primary); }
.gh-name { flex: 1; font-family: 'Cascadia Code', monospace; }
.gh-check { color: var(--color-primary); font-size: 14px; }
.gh-empty { padding: 16px; text-align: center; font-size: 12px; color: var(--color-text-tertiary); }
.gh-actions { display: flex; align-items: center; gap: 8px; }
.gh-sel-count { font-size: 11px; color: var(--color-text-tertiary); }
.gh-spacer { flex: 1; }
</style>
