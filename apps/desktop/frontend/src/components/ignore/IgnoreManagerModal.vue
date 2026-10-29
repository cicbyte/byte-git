<script setup>
/**
 * 模板管理弹窗 — 从 IgnoreView 拆出。
 * 左侧模板列表（本地/GitHub 切换）+ 右侧查看/编辑。
 */
import { ref, computed, watch } from 'vue'
import { message as AMessage } from 'ant-design-vue'
import {
  PlusOutlined, CloudSyncOutlined, ReloadOutlined,
  EditOutlined, DeleteOutlined, EyeOutlined, CopyOutlined,
} from '@ant-design/icons-vue'
import CodeViewer from '@/components/common/CodeViewer.vue'

const app = window.go?.main?.ByteGitApp

const props = defineProps({ open: Boolean })
const emit = defineEmits(['update:open'])

const mgrSource = ref('local')
const mgrEditing = ref(null)
const mgrViewing = ref(null)
const mgrSearch = ref('')
const localTemplates = ref([])
const githubTemplates = ref([])
const githubCloned = ref(false)
const syncLoading = ref(false)

const mgrList = computed(() => {
  const src = mgrSource.value === 'local' ? localTemplates.value : githubTemplates.value
  if (!mgrSearch.value.trim()) return src
  const q = mgrSearch.value.toLowerCase()
  return src.filter(t => t.toLowerCase().includes(q))
})

async function loadLists() {
  try { localTemplates.value = await app.IgnoreListTemplates() } catch { localTemplates.value = [] }
  try {
    const status = await app.IgnoreSourcesStatus()
    const gh = status.find(s => s.name === 'github')
    githubCloned.value = gh?.cloned || false
    if (githubCloned.value) githubTemplates.value = await app.IgnoreListGithubTemplates()
  } catch {}
}

function switchMgrSource(v) { mgrSource.value = v; mgrViewing.value = null; mgrEditing.value = null }

function mgrNew() { mgrEditing.value = { name: '', content: '', isNew: true }; mgrViewing.value = null }

async function mgrView(name) {
  mgrEditing.value = null
  try {
    const content = await app.IgnoreTemplateView(name, mgrSource.value)
    mgrViewing.value = { name, content }
  } catch { AMessage.error('读取失败') }
}

async function mgrEdit(name) {
  mgrViewing.value = null
  try {
    const content = await app.IgnoreTemplateView(name, 'local')
    mgrEditing.value = { name, content, isNew: false }
  } catch { AMessage.error('读取失败') }
}

async function mgrDelete(name) {
  try { await app.IgnoreTemplateDelete(name); AMessage.success('已删除'); await loadLists() }
  catch { AMessage.error('删除失败') }
}

async function mgrSave() {
  const t = mgrEditing.value
  if (!t?.name.trim() || !t?.content.trim()) { AMessage.warning('请填写名称和内容'); return }
  try { await app.IgnoreTemplateSave(t.name.trim(), t.content); AMessage.success('已保存'); mgrEditing.value = null; await loadLists() }
  catch (e) { AMessage.error('保存失败: ' + String(e?.message || e)) }
}

async function mgrCopy(name) {
  try { await app.IgnoreTemplateCopy(name); AMessage.success(`已复制 ${name} 到本地`); await loadLists() }
  catch (e) { AMessage.error('复制失败: ' + String(e?.message || e)) }
}

async function mgrSync() {
  syncLoading.value = true
  try { await app.IgnoreGithubSync(); AMessage.success('同步完成'); await loadLists() }
  catch { AMessage.error('同步失败') }
  finally { syncLoading.value = false }
}

async function mgrReset() {
  try { await app.IgnoreGithubReset(); AMessage.success('已重置'); await loadLists() }
  catch { AMessage.error('重置失败') }
}

watch(() => props.open, (v) => { if (v) loadLists() })
</script>

<template>
  <a-modal :open="open" @update:open="(v) => emit('update:open', v)"
    title="模板管理" width="80%" :footer="null" destroy-on-close>
    <div class="mgr">
      <div class="mgr-left">
        <a-segmented v-model:value="mgrSource" block size="small" :options="[
          { label: '本地', value: 'local' }, { label: 'GitHub', value: 'github' },
        ]" @change="(e) => switchMgrSource(e.target ? e.target.value : e)" />
        <div class="mgr-toolbar">
          <a-input v-model:value="mgrSearch" placeholder="搜索…" size="small" allow-clear class="mgr-search" />
          <a-button v-if="mgrSource === 'local'" size="small" type="primary" @click="mgrNew" title="新建"><PlusOutlined /></a-button>
          <template v-if="mgrSource === 'github'">
            <a-button size="small" @click="mgrSync" :loading="syncLoading" title="同步"><CloudSyncOutlined /></a-button>
            <a-button size="small" danger @click="mgrReset" title="重置"><ReloadOutlined /></a-button>
          </template>
        </div>
        <div class="mgr-status" v-if="mgrSource === 'github'">{{ githubCloned ? `共 ${githubTemplates.length} 个模板` : '未同步，点 ↻ 同步' }}</div>
        <div class="mgr-status" v-else>共 {{ localTemplates.length }} 个模板</div>
        <div class="mgr-list-body">
          <div v-for="t in mgrList" :key="mgrSource+'-'+t" class="mgr-item" :class="{ active: mgrViewing?.name === t || mgrEditing?.name === t }">
            <span class="mgr-item-name" @click="mgrView(t)">{{ t }}</span>
            <div class="mgr-item-acts">
              <a-button size="small" type="text" @click="mgrView(t)" title="查看"><EyeOutlined /></a-button>
              <template v-if="mgrSource === 'local'">
                <a-button size="small" type="text" @click="mgrEdit(t)" title="编辑"><EditOutlined /></a-button>
                <a-button size="small" type="text" danger @click="mgrDelete(t)" title="删除"><DeleteOutlined /></a-button>
              </template>
              <a-button v-else size="small" type="text" @click="mgrCopy(t)" title="复制到本地"><CopyOutlined /></a-button>
            </div>
          </div>
        </div>
      </div>
      <div class="mgr-right">
        <template v-if="mgrEditing">
          <div class="mgr-edit-head">
            <a-input v-model:value="mgrEditing.name" placeholder="模板名称" size="small" style="width:200px" />
            <a-button size="small" type="primary" @click="mgrSave">保存</a-button>
          </div>
          <CodeViewer :code="mgrEditing.content" filename=".gitignore" editable class="mgr-edit-viewer"
            @update:modelValue="(v) => mgrEditing.content = v" />
        </template>
        <template v-else-if="mgrViewing">
          <div class="mgr-view-head">{{ mgrViewing.name }}</div>
          <CodeViewer :code="mgrViewing.content" filename=".gitignore" class="mgr-edit-viewer" />
        </template>
        <div v-else class="mgr-placeholder">选择模板查看或编辑</div>
      </div>
    </div>
  </a-modal>
</template>

<style scoped>
.mgr { display: flex; gap: 12px; height: 480px; }
.mgr-left { width: 280px; flex-shrink: 0; display: flex; flex-direction: column; gap: 8px; }
.mgr-toolbar { display: flex; gap: 4px; }
.mgr-search { flex: 1; }
.mgr-status { font-size: 11px; color: var(--color-text-tertiary); }
.mgr-list-body { flex: 1; overflow-y: auto; border: 1px solid var(--color-border-light); border-radius: var(--radius-sm); }
.mgr-item { display: flex; align-items: center; gap: 4px; padding: 6px 8px; border-bottom: 1px solid var(--color-border-light); }
.mgr-item:hover { background: var(--color-hover); }
.mgr-item.active { background: var(--color-primary-bg); }
.mgr-item-name { flex: 1; font-size: 12px; color: var(--color-text); cursor: pointer; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.mgr-item-acts { display: flex; gap: 2px; flex-shrink: 0; }
.mgr-right { flex: 1; display: flex; flex-direction: column; gap: 8px; min-width: 0; }
.mgr-edit-head { display: flex; gap: 8px; }
.mgr-view-head { font-size: 13px; font-weight: 600; color: var(--color-text); }
.mgr-edit-viewer { flex: 1; min-height: 0; border: 1px solid var(--color-border); border-radius: var(--radius-sm); }
.mgr-placeholder { flex: 1; display: flex; align-items: center; justify-content: center; color: var(--color-text-tertiary); font-size: 13px; }
</style>
