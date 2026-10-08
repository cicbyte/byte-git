<script setup>
/** Wiki 同步 — reference wiki 子系统的管理界面。
 *
 * Wiki 是 reference 的 Git 同步知识库(AI 生成的主题 md 文件,跨项目复用)。
 * 涵盖:状态查看 / 同步(pull+commit+push) / 仅提交 / 远程仓库查看+设置。
 * 回收站功能已移至独立的 TrashSection。
 */
import { ref, computed, onMounted } from 'vue'
import { message } from 'ant-design-vue'
import {
  CloudSyncOutlined, CheckCircleOutlined, CloseCircleOutlined,
  SaveOutlined, ReloadOutlined,
  LinkOutlined,
} from '@ant-design/icons-vue'

const app = window.go?.main?.ByteGitApp

// ---- status ----
const status = ref(null) // WikiStatus | null
const statusLoading = ref(false)

async function loadStatus() {
  if (!app?.ReferenceWikiStatus) return
  statusLoading.value = true
  try {
    status.value = await app.ReferenceWikiStatus()
  } catch (e) {
    message.error('读取 wiki 状态失败: ' + String(e?.message || e))
  } finally {
    statusLoading.value = false
  }
}

// ---- sync / commit ----
const syncing = ref(false)
const committing = ref(false)
const cmdOutput = ref('') // raw text output of last sync/commit

async function doSync() {
  syncing.value = true
  cmdOutput.value = ''
  try {
    message.loading({ content: '同步中(pull + commit + push)...', key: 'wiki-sync', duration: 0 })
    const out = await app.ReferenceWikiSync()
    cmdOutput.value = out || '同步完成'
    message.success({ content: 'Wiki 同步完成', key: 'wiki-sync', duration: 1.5 })
    await loadStatus() // refresh
  } catch (e) {
    message.error({ content: '同步失败: ' + String(e?.message || e), key: 'wiki-sync', duration: 3 })
  } finally {
    syncing.value = false
  }
}

async function doCommit() {
  committing.value = true
  cmdOutput.value = ''
  try {
    const out = await app.ReferenceWikiCommit()
    cmdOutput.value = out || '已提交'
    message.success('已提交本地更改')
    await loadStatus()
  } catch (e) {
    message.error('提交失败: ' + String(e?.message || e))
  } finally {
    committing.value = false
  }
}

// ---- remote ----
const remoteInput = ref('')

async function loadRemote() {
  if (!app?.ReferenceWikiRemoteGet) return
  try {
    remoteInput.value = await app.ReferenceWikiRemoteGet() || ''
  } catch {
    remoteInput.value = ''
  }
}

async function setRemote() {
  const url = remoteInput.value.trim()
  if (!url) {
    message.warning('请输入远程仓库地址')
    return
  }
  try {
    await app.ReferenceWikiRemoteSet(url)
    message.success('远程仓库已设置')
    await loadStatus()
  } catch (e) {
    message.error('设置失败: ' + String(e?.message || e))
  }
}

// lifecycle
onMounted(async () => {
  await loadStatus()
  loadRemote()
})
</script>

<template>
  <div class="section">
    <!-- status + sync card -->
    <div class="sec-card">
      <div class="card-head">
        <div class="card-title">同步状态</div>
        <div class="card-desc">wiki 是 Git 同步的知识库。同步 = pull 远程更新 + 提交本地新增/修改 + push 到远程。</div>
      </div>
      <div class="card-body">
        <!-- status row -->
        <div class="status-row">
          <template v-if="status">
            <component :is="status.initialized ? CheckCircleOutlined : CloseCircleOutlined"
              :class="status.initialized ? 'ok' : 'err'" />
            <span :class="status.initialized ? 'ok' : 'err'" style="font-weight: 600">
              {{ status.initialized ? '已初始化' : '未初始化' }}
            </span>
            <span v-if="status.remote" style="color: var(--color-text-secondary)">
              <LinkOutlined /> {{ status.remote }}
            </span>
            <span v-else class="hint">未设置远程仓库</span>
            <span v-if="status.initialized" :class="status.clean ? 'ok' : 'warn'" style="font-weight: 500">
              {{ status.clean ? '工作区干净' : '有未提交更改' }}
            </span>
          </template>
          <template v-else-if="statusLoading">
            <a-spin size="small" /><span class="hint">读取中...</span>
          </template>
          <span class="spacer"></span>
          <a-button size="small" type="text" @click="loadStatus" title="刷新状态">
            <template #icon><ReloadOutlined /></template>
          </a-button>
        </div>

        <!-- action buttons -->
        <div class="form-row">
          <a-button type="primary" :loading="syncing" :disabled="!status?.initialized" @click="doSync">
            <template #icon><CloudSyncOutlined /></template>同步(pull+commit+push)
          </a-button>
          <a-button :loading="committing" :disabled="!status?.initialized || status?.clean" @click="doCommit">
            <template #icon><SaveOutlined /></template>仅提交本地更改
          </a-button>
          <span class="hint">同步会先拉取远程更新,有冲突时需手动解决</span>
        </div>

        <!-- raw output -->
        <pre v-if="cmdOutput" class="cli-output">{{ cmdOutput }}</pre>
      </div>
    </div>

    <!-- remote config card -->
    <div class="sec-card">
      <div class="card-head">
        <div class="card-title">远程仓库</div>
        <div class="card-desc">wiki 的 Git 远程地址,用于跨设备/跨项目共享知识库。</div>
      </div>
      <div class="card-body">
        <div class="form-row">
          <a-input v-model:value="remoteInput" placeholder="https://github.com/you/reference_wiki.git" style="flex: 1; min-width: 300px" allow-clear />
          <a-button type="primary" @click="setRemote">设置</a-button>
        </div>
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
