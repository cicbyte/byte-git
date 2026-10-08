<script setup>
/**
 * 文件状态检查弹窗 — 从 IgnoreView 拆出。
 * 扫描工作区所有文件，显示 tracked/ignored/untracked 状态。
 */
import { ref, computed, watch } from 'vue'
import { message as AMessage } from 'ant-design-vue'
import FileTreePicker from '../common/FileTreePicker.vue'

const app = window.go?.main?.ByteGitApp

const props = defineProps({ open: Boolean })
const emit = defineEmits(['update:open'])

const checkFiles = ref([])
const checkBadges = ref({})
const checkExcludeSet = new Set(['.git', '.gitignore'])
const checkSearch = ref('')
const filesLoading = ref(false)

const checkFiltered = computed(() => {
  if (!checkSearch.value.trim()) return checkFiles.value
  const q = checkSearch.value.toLowerCase()
  return checkFiles.value.filter(f => f.toLowerCase().includes(q))
})

const statusCounts = computed(() => {
  let tracked = 0, ignored = 0, untracked = 0
  for (const k in checkBadges.value) {
    if (checkBadges.value[k] === 'tracked') tracked++
    else if (checkBadges.value[k] === 'ignored') ignored++
    else untracked++
  }
  return { tracked, ignored, untracked }
})

async function openCheck() {
  if (!app?.IgnoreFileStatuses) return
  filesLoading.value = true
  checkBadges.value = {}
  try {
    const statuses = await app.IgnoreFileStatuses()
    const files = []
    const badges = {}
    for (const s of statuses) {
      files.push(s.path)
      badges[s.path] = s.status
    }
    checkFiles.value = files
    checkBadges.value = badges
  } catch (e) {
    AMessage.error('加载文件状态失败: ' + String(e?.message || e))
    checkFiles.value = []
  } finally {
    filesLoading.value = false
  }
}

watch(() => props.open, (v) => { if (v) openCheck() })
</script>

<template>
  <a-modal :open="open" @update:open="(v) => emit('update:open', v)"
    title="文件状态" width="600px" :footer="null" destroy-on-close>
    <div class="chk-toolbar">
      <span class="chk-legend">
        <span class="lg lg-tracked">✓ {{ statusCounts.tracked }} 跟踪</span>
        <span class="lg lg-ignored">🚫 {{ statusCounts.ignored }} 忽略</span>
        <span class="lg lg-untracked">○ {{ statusCounts.untracked }} 未跟踪</span>
      </span>
      <a-input v-model:value="checkSearch" placeholder="搜索…" size="small" allow-clear class="chk-search" />
    </div>
    <div class="chk-tree-area">
      <div v-if="filesLoading" class="chk-loading"><a-spin tip="扫描文件状态…" /></div>
      <FileTreePicker
        v-else
        :files="checkFiltered"
        :exclude="checkExcludeSet"
        :badges="checkBadges"
      />
    </div>
  </a-modal>
</template>

<style scoped>
.chk-toolbar { display: flex; align-items: center; gap: 8px; margin-bottom: 8px; }
.chk-legend { display: flex; gap: 12px; font-size: 11px; }
.lg-tracked { color: var(--color-success); }
.lg-ignored { color: var(--color-text-tertiary); }
.lg-untracked { color: var(--color-warning); }
.chk-search { width: 140px; }
.chk-tree-area { max-height: 400px; overflow-y: auto; }
.chk-loading { padding: 40px; text-align: center; }
</style>
