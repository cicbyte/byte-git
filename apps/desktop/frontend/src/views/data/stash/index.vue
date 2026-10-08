<script setup>
import { ref, onMounted, onBeforeUnmount, watch, h } from 'vue'
import { message as AMessage, Modal } from 'ant-design-vue'
import {
  InboxOutlined, PushpinOutlined, DeleteOutlined, EyeOutlined,
} from '@ant-design/icons-vue'
import { useRepoStore } from '../../../stores/repo'
import { usePageFooter } from '../../../composables/usePageFooter'

/**
 * Stash 管理 — 列表 / 命名保存 / 弹出 / 删除 / diff 预览。
 * 嵌入 GitManage 页的 tab，或独立路由。
 */
const repo = useRepoStore()
const app = window.go?.main?.ByteGitApp
const { setFooter, clearFooter } = usePageFooter()

const stashList = ref([])
const loading = ref(false)
const saveName = ref('')
const diffVisible = ref(false)
const diffContent = ref('')
const diffLoading = ref(false)

async function loadList() {
  if (!app?.StashList) return
  loading.value = true
  try {
    stashList.value = (await app.StashList()) || []
  } catch { stashList.value = [] }
  finally { loading.value = false }
}

async function doSave() {
  if (!app?.StashSave) return
  try {
    await app.StashSave(saveName.value.trim())
    AMessage.success('已保存 stash')
    saveName.value = ''
    await loadList()
  } catch (e) { AMessage.error('保存失败: ' + String(e?.message || e)) }
}

async function doPop(index) {
  Modal.confirm({
    title: '弹出此 stash？',
    content: `stash@{${index}} 将被弹出并从列表移除`,
    okText: '弹出', cancelText: '取消',
    async onOk() {
      try { await app.StashPop(index); AMessage.success('已弹出'); await loadList() }
      catch (e) { AMessage.error('弹出失败: ' + String(e?.message || e)) }
    },
  })
}

async function doDrop(index) {
  Modal.confirm({
    title: '删除此 stash？', content: `stash@{${index}} 将被永久删除`,
    okText: '删除', okType: 'danger', cancelText: '取消',
    async onOk() {
      try { await app.StashDrop(index); AMessage.success('已删除'); await loadList() }
      catch (e) { AMessage.error('删除失败: ' + String(e?.message || e)) }
    },
  })
}

async function showDiff(index) {
  diffVisible.value = true
  diffLoading.value = true
  diffContent.value = ''
  try { diffContent.value = await app.StashDiff(index) }
  catch (e) { diffContent.value = '加载失败: ' + String(e?.message || e) }
  finally { diffLoading.value = false }
}

const StashFooter = {
  props: ['count'],
  render() {
    return h('div', { style: 'display:flex;align-items:center;gap:8px;width:100%;font-size:12px;color:var(--color-text-tertiary)' }, [
      h(InboxOutlined, { style: 'font-size:13px' }),
      h('span', { style: 'font-weight:500;color:var(--color-text)' }, 'Stash 管理'),
      h('span', `${this.count} 条 stash`),
    ])
  },
}

onMounted(async () => {
  await loadList()
  setFooter(StashFooter, () => ({ count: stashList.value.length }))
})
onBeforeUnmount(() => clearFooter())

watch(() => repo.repoEpoch, loadList)
</script>

<template>
  <div class="sv-stash">
    <!-- save bar -->
    <div class="save-bar">
      <a-input v-model:value="saveName" placeholder="命名保存 stash（可选）" size="small" allow-clear
        style="width: 220px" @keydown.enter="doSave" />
      <a-button size="small" @click="doSave">
        <template #icon><PushpinOutlined /></template>保存
      </a-button>
    </div>

    <!-- list -->
    <div class="stash-list">
      <div v-if="loading" class="stash-center"><a-spin size="small" /></div>
      <div v-else-if="!stashList.length" class="stash-empty">
        <InboxOutlined style="font-size:36px;opacity:0.3" />
        <div style="margin-top:8px">没有 stash 记录</div>
      </div>
      <div v-for="s in stashList" :key="s.index" class="stash-card">
        <div class="stash-main">
          <span class="stash-ref">stash@{{ '{' + s.index + '}' }}</span>
          <span v-if="s.name" class="stash-name">{{ s.name }}</span>
          <span class="stash-msg">{{ s.message }}</span>
        </div>
        <div class="stash-actions">
          <button class="stash-btn" @click="showDiff(s.index)" title="查看 diff"><EyeOutlined /></button>
          <button class="stash-btn" @click="doPop(s.index)" title="弹出"><PushpinOutlined /></button>
          <button class="stash-btn danger" @click="doDrop(s.index)" title="删除"><DeleteOutlined /></button>
        </div>
      </div>
    </div>

    <!-- diff modal -->
    <a-modal v-model:open="diffVisible" title="Stash Diff" width="720px" :footer="null">
      <div v-if="diffLoading" style="padding:40px;text-align:center"><a-spin /></div>
      <pre v-else class="stash-diff">{{ diffContent }}</pre>
    </a-modal>
  </div>
</template>

<style scoped>
.sv-stash { display: flex; flex-direction: column; gap: var(--spacing-sm); height: 100%; }
.save-bar { display: flex; gap: 8px; flex-shrink: 0; }
.stash-list { flex: 1; overflow-y: auto; display: flex; flex-direction: column; gap: 4px; }
.stash-center { padding: 40px; text-align: center; }
.stash-empty { padding: 40px; text-align: center; color: var(--color-text-tertiary); font-size: 13px; }
.stash-card {
  display: flex; align-items: center; gap: 8px;
  padding: 8px 12px; border-radius: var(--radius-sm);
  background: var(--color-surface); border: 1px solid var(--color-border);
  transition: all var(--transition-fast);
}
.stash-card:hover { border-color: var(--color-primary); }
.stash-main { flex: 1; min-width: 0; display: flex; align-items: center; gap: 8px; }
.stash-ref { font-family: 'Cascadia Code', monospace; font-size: 11px; color: var(--color-hash); flex-shrink: 0; }
.stash-name { font-size: 13px; color: var(--color-primary); font-weight: 500; flex-shrink: 0; }
.stash-msg { font-size: 12px; color: var(--color-text-secondary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.stash-actions { display: flex; gap: 2px; flex-shrink: 0; }
.stash-btn {
  -webkit-appearance: none; appearance: none;
  display: inline-flex; align-items: center; justify-content: center;
  flex-shrink: 0; width: 28px; height: 28px; min-width: 28px; max-width: 28px;
  padding: 0; border: none; border-radius: var(--radius-xs);
  background: transparent; color: var(--color-text-tertiary);
  cursor: pointer; font-size: 13px; line-height: 1; box-sizing: border-box;
  transition: all var(--transition-fast);
}
.stash-btn:hover { background: var(--color-hover); color: var(--color-text); }
.stash-btn.danger:hover { color: var(--color-error); }
.stash-diff {
  font-family: 'Cascadia Code', monospace; font-size: 12px; line-height: 1.6;
  white-space: pre-wrap; word-break: break-word;
  max-height: 60vh; overflow-y: auto; margin: 0;
  background: var(--color-background); padding: var(--spacing-md); border-radius: var(--radius-sm);
}
</style>
