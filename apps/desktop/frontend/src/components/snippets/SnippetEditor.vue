<script setup>
/**
 * Snippet add/edit form — used inside a modal (from CommandPalette or the
 * management page). Emits 'save' with the snippet data; the parent decides
 * whether to call store.add or store.update.
 */
import { ref, watch, computed } from 'vue'
import { message as AMessage } from 'ant-design-vue'
import { useSnippetsStore } from '../../stores/snippets'

const props = defineProps({
  // existing snippet to edit (null = new mode)
  snippet: { type: Object, default: null },
})
const emit = defineEmits(['save', 'cancel'])

const store = useSnippetsStore()

const form = ref({ command: '', note: '', tag: '', favorite: false })
const saving = ref(false)

const isEdit = computed(() => !!props.snippet)

// populate form when the snippet prop changes (modal open)
watch(
  () => props.snippet,
  (s) => {
    form.value = {
      command: s?.command || '',
      note: s?.note || '',
      tag: s?.tag || '',
      favorite: s?.favorite || false,
    }
  },
  { immediate: true },
)

async function onSave() {
  if (!form.value.command.trim()) {
    AMessage.warning('请输入命令')
    return
  }
  saving.value = true
  emit('save', { ...form.value })
  saving.value = false
}
</script>

<template>
  <div class="se-form">
    <div class="se-row">
      <label class="se-label">命令 *</label>
      <a-textarea
        v-model:value="form.command"
        placeholder="git rebase -i HEAD~3"
        :auto-size="{ minRows: 1, maxRows: 6 }"
        class="se-cmd"
      />
      <span class="se-template-hint">支持变量: <code>{<!-- -->{branch}}</code> <code>{<!-- -->{repo}}</code> <code>{<!-- -->{remote}}</code>（执行时自动替换为当前值）</span>
    </div>
    <div class="se-row">
      <label class="se-label">备注</label>
      <a-input
        v-model:value="form.note"
        placeholder="这条命令是干嘛的，什么时候用"
        allow-clear
      />
    </div>
    <div class="se-row">
      <label class="se-label">分类</label>
      <a-auto-complete
        v-model:value="form.tag"
        :options="store.tags.map((t) => ({ label: t, value: t }))"
        placeholder="分支 / 提交 / 撤销 / 远程 ..."
        allow-clear
        :filter-option="(input, opt) => opt.label.toLowerCase().includes(input.toLowerCase())"
      />
    </div>
    <div class="se-row se-row-inline">
      <label class="se-label">收藏</label>
      <a-switch v-model:checked="form.favorite" />
      <span class="se-hint">收藏的命令在搜索结果中置顶</span>
    </div>
    <div class="se-foot">
      <a-button size="small" @click="emit('cancel')">取消</a-button>
      <a-button size="small" type="primary" :loading="saving" @click="onSave">
        {{ isEdit ? '保存' : '添加' }}
      </a-button>
    </div>
  </div>
</template>

<style scoped>
.se-form { display: flex; flex-direction: column; gap: 14px; }
.se-row { display: flex; flex-direction: column; gap: 4px; }
.se-row-inline { flex-direction: row; align-items: center; gap: 10px; }
.se-label { font-size: 12px; font-weight: 600; color: var(--color-text-secondary); }
.se-cmd :deep(textarea) { font-family: 'Cascadia Code', 'JetBrains Mono', monospace; font-size: 13px; }
.se-template-hint { font-size: 11px; color: var(--color-text-tertiary); margin-top: 2px; }
.se-template-hint code { font-family: 'Cascadia Code', monospace; color: var(--color-primary); background: var(--color-primary-bg); padding: 0 4px; border-radius: 3px; }
.se-hint { font-size: 11px; color: var(--color-text-tertiary); margin-left: 8px; }
.se-foot { display: flex; justify-content: flex-end; gap: 8px; margin-top: 4px; }
</style>
