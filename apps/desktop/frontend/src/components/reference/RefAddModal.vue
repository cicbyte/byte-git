<script setup>
/**
 * 添加引用仓库弹窗 — 远程 URL / 本地路径两种模式。
 * 提交后由父组件调用后端 ReferenceAdd 并处理结果。
 */
import { ref, watch, computed } from 'vue'
import { message } from 'ant-design-vue'
import { PlusOutlined, LinkOutlined, FolderOpenOutlined } from '@ant-design/icons-vue'

const props = defineProps({
  open: { type: Boolean, default: false },
  loading: { type: Boolean, default: false },
})
const emit = defineEmits(['update:open', 'submit'])

const local = ref(false)
const target = ref('')
const branch = ref('')
const name = ref('')
const update = ref(false)
const fileInput = ref(null)

// reset fields each time the modal opens
watch(() => props.open, (v) => {
  if (v) {
    local.value = false
    target.value = ''
    branch.value = ''
    name.value = ''
    update.value = false
  }
})

const placeholder = computed(() =>
  local.value
    ? 'D:\\code\\my-lib  或  ~/projects/my-lib'
    : 'https://github.com/owner/repo  或  owner/repo',
)

function pickLocal() {
  // hidden <input type="file"> with webkitdirectory picks a folder; but wails
  // file dialog may not honour webkitdirectory, so we accept any file from the
  // dir as a hint and strip to the parent path.
  if (fileInput.value) fileInput.value.click()
}

function onPickFile(e) {
  const f = e.target.files?.[0]
  if (f) {
    // f.path is absolute on wails/electron; take the directory part
    const p = f.path || f.name
    const slash = Math.max(p.lastIndexOf('/'), p.lastIndexOf('\\'))
    target.value = slash >= 0 ? p.slice(0, slash) : p
    local.value = true
  }
  e.target.value = ''
}

function submit() {
  const t = target.value.trim()
  if (!t) {
    message.warning('请填写仓库 URL 或本地路径')
    return
  }
  emit('submit', {
    target: t,
    local: local.value,
    branch: branch.value.trim(),
    name: name.value.trim(),
    update: update.value,
  })
}

function close() { emit('update:open', false) }
</script>

<template>
  <a-modal
    :open="open"
    title="添加引用仓库"
    width="560px"
    :footer="null"
    destroy-on-close
    @update:open="(v) => emit('update:open', v)"
  >
    <!-- mode switch -->
    <div class="mode-row">
      <button class="mode-btn" :class="{ on: !local }" @click="local = false">
        <LinkOutlined /> 远程仓库
      </button>
      <button class="mode-btn" :class="{ on: local }" @click="local = true">
        <FolderOpenOutlined /> 本地仓库
      </button>
    </div>

    <!-- target -->
    <div class="field">
      <label>{{ local ? '本地路径' : '仓库 URL' }}</label>
      <div class="target-row">
        <a-input
          v-model:value="target"
          :placeholder="placeholder"
          allow-clear
          @press-enter="submit"
        />
        <a-button v-if="local" @click="pickLocal" title="选择目录">
          <template #icon><FolderOpenOutlined /></template>
        </a-button>
      </div>
      <input ref="fileInput" type="file" style="display:none" @change="onPickFile" />
    </div>

    <!-- branch (remote only) -->
    <div class="field" v-if="!local">
      <label>分支 / 标签（可选）</label>
      <a-input v-model:value="branch" placeholder="留空 = 默认分支" allow-clear />
    </div>

    <!-- name -->
    <div class="field">
      <label>引用名（可选）</label>
      <a-input v-model:value="name" placeholder="留空 = 自动从 URL/路径推断" allow-clear />
    </div>

    <!-- update flag -->
    <div class="field" v-if="!local">
      <a-checkbox v-model:checked="update">
        若缓存已存在则强制 git pull 更新
      </a-checkbox>
    </div>

    <!-- footer -->
    <div class="foot">
      <span class="hint">
        {{ local ? '本地引用只创建链接，不复制代码' : '远程引用会克隆完整历史到全局缓存' }}
      </span>
      <span class="spacer"></span>
      <a-button @click="close">取消</a-button>
      <a-button type="primary" :loading="loading" @click="submit">
        <template #icon><PlusOutlined /></template>添加
      </a-button>
    </div>
  </a-modal>
</template>

<style scoped>
.mode-row { display: flex; gap: 8px; margin-bottom: 16px; }
.mode-btn {
  flex: 1; padding: 10px 12px; cursor: pointer;
  border: 1px solid var(--color-border); border-radius: var(--radius-sm);
  background: var(--color-background); color: var(--color-text-secondary);
  font-size: 13px; transition: all var(--transition-fast);
  display: flex; align-items: center; justify-content: center; gap: 6px;
}
.mode-btn:hover { color: var(--color-primary); border-color: var(--color-primary); }
.mode-btn.on { background: var(--color-primary); border-color: var(--color-primary); color: #fff; }

.field { margin-bottom: 14px; }
.field label { display: block; font-size: 12px; color: var(--color-text-tertiary); margin-bottom: 6px; }
.target-row { display: flex; gap: 8px; }

.foot { display: flex; align-items: center; gap: 10px; margin-top: 18px; padding-top: 14px; border-top: 1px solid var(--color-border-light); }
.spacer { flex: 1; }
.hint { font-size: 11px; color: var(--color-text-tertiary); }
</style>
