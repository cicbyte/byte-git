<script setup>
import { ref, computed, onMounted, watch, onBeforeUnmount, h } from 'vue'
import { useRouter } from 'vue-router'
import { message as AMessage, Modal } from 'ant-design-vue'
import {
  GithubOutlined, ReloadOutlined, PlusOutlined, DeleteOutlined,
  UploadOutlined, TagOutlined, ExclamationCircleOutlined,
  CheckCircleOutlined, PaperClipOutlined,
} from '@ant-design/icons-vue'
import { MdEditor } from 'md-editor-v3'
import 'md-editor-v3/lib/style.css'
import { useRepoStore } from '../../../stores/repo'
import { usePageFooter } from '../../../composables/usePageFooter'
import { useGitHubCache } from '../../../composables/useGitHub'
import { useThemeStore } from '../../../stores/theme'

const app = window.go?.main?.ByteGitApp
const repo = useRepoStore()
const router = useRouter()
const { status, ensureStatusLoaded } = useGitHubCache()
const { setFooter, clearFooter } = usePageFooter()
const themeStore = useThemeStore()
const mdTheme = computed(() => (themeStore.isDark ? 'dark' : 'light'))

const releases = ref([])
const loading = ref(false)
const acting = ref(false)

// create modal
const createOpen = ref(false)
const form = ref({ tag: '', title: '', body: '', isDraft: false, isPre: false })
const creating = ref(false)

// upload modal
const uploadOpen = ref(false)
const uploadReleaseID = ref(0)
const uploadPath = ref('')
const uploading = ref(false)

const canUse = computed(() => status.value.authed && status.value.remoteIsGh)

async function loadReleases() {
  if (!canUse.value || !app?.GhReleaseList) return
  loading.value = true
  try {
    releases.value = (await app.GhReleaseList()) || []
  } catch (e) {
    AMessage.error('加载 Release 失败: ' + String(e?.message || e))
    releases.value = []
  } finally {
    loading.value = false
  }
}

function updateFooter() {
  if (!canUse.value) return
  setFooter(ReleaseFooter, () => ({
    repo: status.value.ownerRepo,
    count: releases.value.length,
  }))
}

const ReleaseFooter = {
  props: ['repo', 'count'],
  render() {
    return h('div', { style: 'display:flex;align-items:center;gap:8px;width:100%;font-size:12px;color:var(--color-text-tertiary)' }, [
      h(GithubOutlined, { style: 'font-size:13px' }),
      h('span', { style: 'font-weight:500;color:var(--color-text)' }, 'Releases'),
      h('span', this.repo),
      h('div', { style: 'flex:1' }),
      h('span', `${this.count} 个 Release`),
    ])
  },
}

function openCreate() {
  form.value = { tag: '', title: '', body: '', isDraft: false, isPre: false }
  createOpen.value = true
}

async function doCreate() {
  if (!form.value.tag.trim()) { AMessage.warning('请输入 Tag'); return }
  creating.value = true
  try {
    await app.GhReleaseCreate(form.value.tag.trim(), form.value.title.trim(), form.value.body, form.value.isDraft, form.value.isPre)
    AMessage.success(`已创建 Release ${form.value.tag}`)
    createOpen.value = false
    await loadReleases()
  } catch (e) {
    AMessage.error('创建失败: ' + String(e?.message || e))
  } finally {
    creating.value = false
  }
}

function confirmDelete(rel) {
  Modal.confirm({
    title: `删除 Release ${rel.tagName}?`,
    content: '此操作不可撤销。',
    okText: '删除', okType: 'danger', cancelText: '取消',
    async onOk() {
      acting.value = true
      try { await app.GhReleaseDelete(rel.id); AMessage.success('已删除'); await loadReleases() }
      catch (e) { AMessage.error('删除失败: ' + String(e?.message || e)) }
      finally { acting.value = false }
    },
  })
}

function openUpload(rel) {
  uploadReleaseID.value = rel.id
  uploadPath.value = ''
  uploadOpen.value = true
}

function pickFile() {
  // use hidden input for file selection
  const input = document.createElement('input')
  input.type = 'file'
  input.onchange = (e) => {
    const f = e.target.files?.[0]
    if (f) uploadPath.value = f.path
  }
  input.click()
}

async function doUpload() {
  if (!uploadPath.value) { AMessage.warning('请选择文件'); return }
  uploading.value = true
  try {
    await app.GhReleaseUploadAsset(uploadReleaseID.value, uploadPath.value)
    AMessage.success('上传成功')
    uploadOpen.value = false
    await loadReleases()
  } catch (e) {
    AMessage.error('上传失败: ' + String(e?.message || e))
  } finally {
    uploading.value = false
  }
}

function confirmDeleteAsset(rel, asset) {
  Modal.confirm({
    title: `删除资产 ${asset.name}?`,
    okText: '删除', okType: 'danger', cancelText: '取消',
    async onOk() {
      acting.value = true
      try { await app.GhReleaseDeleteAsset(asset.id); AMessage.success('已删除'); await loadReleases() }
      catch (e) { AMessage.error('删除失败: ' + String(e?.message || e)) }
      finally { acting.value = false }
    },
  })
}

function fmtSize(bytes) {
  if (!bytes) return '0 B'
  if (bytes < 1024) return bytes + ' B'
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB'
  return (bytes / 1024 / 1024).toFixed(1) + ' MB'
}

onMounted(async () => {
  await ensureStatusLoaded()
  if (canUse.value) { await loadReleases(); updateFooter() }
})
onBeforeUnmount(() => clearFooter())

watch(() => repo.repoEpoch, () => { loadReleases() })
</script>

<template>
  <div class="gr-view">
    <!-- not ready -->
    <div v-if="!status.tokenSet" class="gr-guide">
      <GithubOutlined class="gr-guide-icon" />
      <div class="gr-guide-title">需要配置 GitHub Token</div>
      <a-button type="primary" size="small" @click="router.push('/settings')">前往设置</a-button>
    </div>
    <div v-else-if="!status.authed" class="gr-guide">
      <ExclamationCircleOutlined class="gr-guide-icon warn" />
      <div class="gr-guide-title">Token 无效</div>
      <a-button type="primary" size="small" @click="router.push('/settings')">前往设置</a-button>
    </div>
    <div v-else-if="!status.remoteIsGh" class="gr-guide">
      <ExclamationCircleOutlined class="gr-guide-icon warn" />
      <div class="gr-guide-title">当前仓库不是 GitHub 仓库</div>
    </div>

    <!-- ready -->
    <div v-else class="gr-ready">
      <div class="gr-head">
        <span class="gr-repo">{{ status.ownerRepo }}</span>
        <a-button size="small" @click="openCreate">
          <template #icon><PlusOutlined /></template>创建
        </a-button>
        <a-button size="small" type="text" :loading="loading" @click="loadReleases" title="刷新">
          <template #icon><ReloadOutlined /></template>
        </a-button>
      </div>

      <div class="gr-list">
        <div v-if="loading" class="gr-center"><a-spin /></div>
        <div v-else-if="!releases.length" class="gr-empty">
          <TagOutlined style="font-size:32px;opacity:0.3" />
          <div style="margin-top:8px">没有 Release</div>
        </div>
        <div v-for="rel in releases" :key="rel.id" class="gr-card">
          <div class="gr-card-head">
            <div class="gr-tag-row">
              <span class="gr-tag">{{ rel.tagName }}</span>
              <span v-if="rel.draft" class="gr-badge draft">Draft</span>
              <span v-if="rel.preRelease" class="gr-badge pre">Pre-release</span>
              <span class="gr-date">{{ rel.createdAt }}</span>
            </div>
            <div class="gr-card-actions">
              <a-button size="small" type="text" @click="openUpload(rel)" title="上传资产">
                <UploadOutlined />
              </a-button>
              <a-button size="small" type="text" @click="confirmDelete(rel)" title="删除">
                <DeleteOutlined />
              </a-button>
            </div>
          </div>
          <div v-if="rel.name" class="gr-name">{{ rel.name }}</div>
          <!-- assets -->
          <div v-if="rel.assets?.length" class="gr-assets">
            <div v-for="a in rel.assets" :key="a.id" class="gr-asset">
              <PaperClipOutlined />
              <span class="gr-asset-name" :title="a.name">{{ a.name }}</span>
              <span class="gr-asset-size">{{ fmtSize(a.size) }}</span>
              <button class="gr-asset-del" @click="confirmDeleteAsset(rel, a)" title="删除"><DeleteOutlined /></button>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- create modal -->
    <a-modal v-model:open="createOpen" title="创建 Release" width="680px" :footer="null" destroy-on-close>
      <div class="gr-create">
        <div style="display:flex;gap:12px;margin-bottom:12px">
          <a-input v-model:value="form.tag" placeholder="Tag (如 v1.0.0)" style="width:200px" />
          <a-input v-model:value="form.title" placeholder="标题（可选）" style="flex:1" />
        </div>
        <div class="gr-md-label">发布说明（Markdown）</div>
        <MdEditor v-model="form.body" :theme="mdTheme" language="zh-CN"
          :toolbars="['bold','italic','strike','quote','list','orderedList','code','codeRow','link']"
          :preview="true" style="height:240px;margin-bottom:12px;border-radius:var(--radius-sm);overflow:hidden;" />
        <div style="display:flex;align-items:center;gap:16px;margin-bottom:12px">
          <a-checkbox v-model:checked="form.isDraft">草稿</a-checkbox>
          <a-checkbox v-model:checked="form.isPre">预发布</a-checkbox>
        </div>
        <div style="display:flex;justify-content:flex-end;gap:8px">
          <a-button size="small" @click="createOpen = false">取消</a-button>
          <a-button size="small" type="primary" :loading="creating" @click="doCreate">创建</a-button>
        </div>
      </div>
    </a-modal>

    <!-- upload modal -->
    <a-modal v-model:open="uploadOpen" title="上传资产" width="480px" :footer="null" destroy-on-close>
      <div class="gr-upload">
        <a-input :value="uploadPath" placeholder="选择文件…" readonly style="margin-bottom:12px">
          <template #suffix>
            <a-button size="small" type="text" @click="pickFile"><UploadOutlined /></a-button>
          </template>
        </a-input>
        <div style="display:flex;justify-content:flex-end;gap:8px">
          <a-button size="small" @click="uploadOpen = false">取消</a-button>
          <a-button size="small" type="primary" :loading="uploading" :disabled="!uploadPath" @click="doUpload">上传</a-button>
        </div>
      </div>
    </a-modal>
  </div>
</template>

<style scoped>
.gr-view { width: 100%; height: 100%; display: flex; flex-direction: column; background: var(--color-background); padding: var(--spacing-md); overflow: hidden; }

.gr-guide { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 10px; }
.gr-guide-icon { font-size: 48px; color: var(--color-text-tertiary); opacity: 0.4; }
.gr-guide-icon.warn { color: var(--color-warning); opacity: 0.7; }
.gr-guide-title { font-size: 16px; font-weight: 600; color: var(--color-text); }

.gr-ready { flex: 1; display: flex; flex-direction: column; min-height: 0; }
.gr-head { display: flex; align-items: center; gap: 8px; margin-bottom: var(--spacing-sm); flex-shrink: 0; }
.gr-repo { font-family: 'Cascadia Code', monospace; font-size: 14px; font-weight: 600; color: var(--color-text); margin-right: auto; }

.gr-list { flex: 1; overflow-y: auto; display: flex; flex-direction: column; gap: 8px; }
.gr-center { display: flex; align-items: center; justify-content: center; height: 200px; }
.gr-empty { display: flex; flex-direction: column; align-items: center; justify-content: center; color: var(--color-text-tertiary); font-size: 13px; height: 200px; }

.gr-card {
  padding: 12px 16px; border-radius: var(--radius-sm);
  background: var(--color-surface); border: 1px solid var(--color-border);
  border-left: 3px solid var(--color-primary);
}
.gr-card-head { display: flex; align-items: center; justify-content: space-between; }
.gr-tag-row { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
.gr-tag { font-family: 'Cascadia Code', monospace; font-size: 14px; font-weight: 700; color: var(--color-primary); }
.gr-badge { font-size: 9px; padding: 1px 6px; border-radius: 3px; font-weight: 600; }
.gr-badge.draft { background: var(--color-warning-bg); color: var(--color-warning); }
.gr-badge.pre { background: var(--color-info-bg); color: var(--color-info); }
.gr-date { font-size: 11px; color: var(--color-text-tertiary); }
.gr-card-actions { display: flex; gap: 2px; flex-shrink: 0; }
.gr-name { font-size: 13px; color: var(--color-text); margin-top: 6px; }

.gr-assets { margin-top: 8px; display: flex; flex-direction: column; gap: 3px; }
.gr-asset {
  display: flex; align-items: center; gap: 6px;
  padding: 4px 8px; border-radius: var(--radius-xs);
  background: var(--color-background); font-size: 12px;
}
.gr-asset-name { flex: 1; color: var(--color-text-secondary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-family: 'Cascadia Code', monospace; }
.gr-asset-size { color: var(--color-text-tertiary); font-size: 11px; flex-shrink: 0; }
.gr-asset-del {
  -webkit-appearance: none; appearance: none; border: none; background: transparent;
  color: var(--color-text-tertiary); cursor: pointer; font-size: 12px; padding: 2px; flex-shrink: 0;
}
.gr-asset-del:hover { color: var(--color-error); }

.gr-create { display: flex; flex-direction: column; }
.gr-md-label { font-size: 12px; font-weight: 600; color: var(--color-text-secondary); margin-bottom: 4px; }
.gr-upload { display: flex; flex-direction: column; }
</style>
