<script setup>
/**
 * 文件状态检查弹窗 — 从 IgnoreView 拆出。
 * 扫描工作区所有文件，显示 tracked/ignored/untracked 状态。
 */
import { ref, computed, watch } from 'vue'
import { message as AMessage } from 'ant-design-vue'
import { CheckOutlined, StopOutlined } from '@ant-design/icons-vue'
import FileTreePicker from '@/components/common/FileTreePicker.vue'

const app = window.go?.main?.ByteGitApp

const props = defineProps({ open: Boolean })
const emit = defineEmits(['update:open'])

const checkFiles = ref([])
const checkBadges = ref({})
// 折叠忽略目录（node_modules 等）是目录条目——不传 dirSet 会被当文件渲染成重复叶节点
const checkDirSet = ref(new Set())
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
    const dirs = new Set()
    for (const s of statuses) {
      files.push(s.path)
      badges[s.path] = s.status
      if (s.isDir) dirs.add(s.path)
    }
    checkFiles.value = files
    checkBadges.value = badges
    checkDirSet.value = dirs
  } catch (e) {
    AMessage.error('加载文件状态失败: ' + String(e?.message || e))
    checkFiles.value = []
  } finally {
    filesLoading.value = false
  }
}

// 右键「检查是否被忽略」→ 查命中的规则并提示
async function onCheckFile(path) {
  if (!app?.IgnoreCheck) return
  try {
    const res = await app.IgnoreCheck(path)
    if (res.ignored) {
      AMessage.warning(`「${path}」被忽略 · 规则 ${res.rule}（${res.source}）`)
    } else {
      AMessage.success(`「${path}」未被忽略`)
    }
  } catch (e) {
    AMessage.error('检查失败: ' + String(e?.message || e))
  }
}

watch(() => props.open, (v) => { if (v) openCheck() })
</script>

<template>
  <a-modal :open="open" @update:open="(v) => emit('update:open', v)"
    title="文件状态" width="600px" :footer="null" destroy-on-close>
    <div class="chk-toolbar">
      <span class="chk-legend">
        <span class="lg lg-tracked"><CheckOutlined /> {{ statusCounts.tracked }} 跟踪</span>
        <span class="lg lg-ignored"><StopOutlined /> {{ statusCounts.ignored }} 忽略</span>
        <span class="lg lg-untracked"><span class="lg-dot" /> {{ statusCounts.untracked }} 未跟踪</span>
      </span>
      <div class="chk-spacer" />
      <a-input v-model:value="checkSearch" placeholder="搜索文件…" size="small" allow-clear class="chk-search" />
    </div>
    <div class="chk-tree-area">
      <div v-if="filesLoading" class="chk-loading"><a-spin tip="扫描文件状态…" /></div>
      <FileTreePicker
        v-else
        :files="checkFiltered"
        :exclude="checkExcludeSet"
        :badges="checkBadges"
        :dir-set="checkDirSet"
        @check-file="onCheckFile"
      />
    </div>
  </a-modal>
</template>

<style scoped>
.chk-toolbar { display: flex; align-items: center; gap: 8px; margin-bottom: 8px; }
.chk-legend { display: flex; gap: 14px; font-size: 11px; }
.lg { display: inline-flex; align-items: center; gap: 4px; }
.lg-dot { width: 8px; height: 8px; border: 1.5px solid currentColor; border-radius: 50%; }
.lg-tracked { color: var(--color-success); }
.lg-ignored { color: var(--color-error); }
.lg-untracked { color: var(--color-warning); }
.chk-spacer { flex: 1; }
.chk-search { width: 180px; }
.chk-tree-area { max-height: 400px; overflow-y: auto; }
.chk-loading { padding: 40px; text-align: center; }
</style>
