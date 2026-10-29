<script setup>
/**
 * .gitignore 模板面板 — 本地/GitHub 双来源。
 * 左列表（来源切换/搜索/新建/同步）+ 右查看/编辑，删除带确认。
 */
import { ref, computed, onMounted } from 'vue'
import { message as AMessage, Modal } from 'ant-design-vue'
import {
  PlusOutlined, CloudSyncOutlined, ReloadOutlined,
  EditOutlined, DeleteOutlined, EyeOutlined, CopyOutlined,
} from '@ant-design/icons-vue'
import CodeViewer from '@/components/common/CodeViewer.vue'

const app = window.go?.main?.ByteGitApp

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

function switchMgrSource(v) {
  mgrSource.value = v
  mgrViewing.value = null
  mgrEditing.value = null
}

function mgrNew() {
  mgrEditing.value = { name: '', content: '', isNew: true }
  mgrViewing.value = null
}

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

function confirmDelete(name) {
  Modal.confirm({
    title: '确认删除模板？',
    content: `将删除本地模板「${name}」。`,
    okText: '删除', okType: 'danger', cancelText: '取消',
    async onOk() {
      try { await app.IgnoreTemplateDelete(name); AMessage.success('已删除'); await loadLists() }
      catch { AMessage.error('删除失败') }
    },
  })
}

async function mgrSave() {
  const t = mgrEditing.value
  if (!t?.name.trim() || !t?.content.trim()) { AMessage.warning('请填写名称和内容'); return }
  try {
    await app.IgnoreTemplateSave(t.name.trim(), t.content)
    AMessage.success('已保存')
    mgrEditing.value = null
    await loadLists()
  } catch (e) { AMessage.error('保存失败: ' + String(e?.message || e)) }
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

onMounted(loadLists)
</script>

<template>
  <div class="tp-pane">
    <!-- 左：列表 -->
    <div class="tp-side">
      <div class="tp-toolbar">
        <a-segmented
          :value="mgrSource" block size="small"
          :options="[{ label: '本地', value: 'local' }, { label: 'GitHub', value: 'github' }]"
          @change="(e) => switchMgrSource(e.target ? e.target.value : e)"
        />
        <div class="tp-toolbar-row">
          <a-input v-model:value="mgrSearch" placeholder="搜索模板…" size="small" allow-clear class="tp-search" />
          <a-button v-if="mgrSource === 'local'" size="small" type="primary" @click="mgrNew" title="新建模板"><PlusOutlined /></a-button>
          <template v-else>
            <a-button size="small" @click="mgrSync" :loading="syncLoading" title="同步 GitHub 模板库"><CloudSyncOutlined /></a-button>
            <a-button size="small" danger @click="mgrReset" title="重置"><ReloadOutlined /></a-button>
          </template>
        </div>
        <div class="tp-count">
          {{ mgrSource === 'github'
            ? (githubCloned ? `共 ${githubTemplates.length} 个模板` : '未同步，点 ↻ 同步')
            : `共 ${localTemplates.length} 个模板` }}
        </div>
      </div>
      <div class="tp-list">
        <div
          v-for="t in mgrList"
          :key="mgrSource + '-' + t"
          class="tp-item"
          :class="{ active: mgrViewing?.name === t || mgrEditing?.name === t }"
          @click="mgrView(t)"
        >
          <span class="tp-item-name">{{ t }}</span>
          <div class="tp-item-acts" @click.stop>
            <a-button size="small" type="text" @click="mgrView(t)" title="查看"><EyeOutlined /></a-button>
            <template v-if="mgrSource === 'local'">
              <a-button size="small" type="text" @click="mgrEdit(t)" title="编辑"><EditOutlined /></a-button>
              <a-button size="small" type="text" danger @click="confirmDelete(t)" title="删除"><DeleteOutlined /></a-button>
            </template>
            <a-button v-else size="small" type="text" @click="mgrCopy(t)" title="复制到本地"><CopyOutlined /></a-button>
          </div>
        </div>
        <div v-if="!mgrList.length" class="tp-empty-list">无匹配模板</div>
      </div>
    </div>

    <!-- 右：查看/编辑 -->
    <div class="tp-main">
      <template v-if="mgrEditing">
        <div class="tp-edit-head">
          <span class="tp-edit-tag">{{ mgrEditing.isNew ? '新建' : '编辑' }}</span>
          <a-input v-model:value="mgrEditing.name" placeholder="模板名称（如 python）" size="small" class="tp-name-input" />
          <a-button size="small" @click="mgrEditing = null">取消</a-button>
          <a-button size="small" type="primary" @click="mgrSave">保存</a-button>
        </div>
        <div class="tp-editor">
          <CodeViewer
            :code="mgrEditing.content"
            filename=".gitignore"
            editable
            class="tp-code"
            @update:modelValue="(v) => mgrEditing.content = v"
          />
        </div>
      </template>
      <template v-else-if="mgrViewing">
        <div class="tp-view-head">{{ mgrViewing.name }}<span class="tp-view-sub">{{ mgrSource === 'github' ? 'GitHub 模板（只读，可复制到本地）' : '本地模板' }}</span></div>
        <div class="tp-editor">
          <CodeViewer :code="mgrViewing.content" filename=".gitignore" class="tp-code" />
        </div>
      </template>
      <div v-else class="tp-placeholder">从左侧选择模板查看，或点 + 新建</div>
    </div>
  </div>
</template>

<style scoped>
@import './pane.css';
</style>
