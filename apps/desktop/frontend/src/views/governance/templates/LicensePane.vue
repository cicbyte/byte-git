<script setup>
/**
 * LICENSE 模板面板 — 用户许可证模板的查看/编辑/删除/新建。
 * 模板存于 ~/.cicbyte/apps/byte-git/licenses/（首次自动从内嵌模板播种）；
 * 编辑后的版本在许可证页创建时优先使用。
 */
import { ref, computed, onMounted } from 'vue'
import { message as AMessage, Modal } from 'ant-design-vue'
import {
  PlusOutlined, EditOutlined, DeleteOutlined,
} from '@ant-design/icons-vue'
import CodeViewer from '@/components/common/CodeViewer.vue'

const app = window.go?.main?.ByteGitApp

const editing = ref(null)
const viewing = ref(null)
const search = ref('')
const templates = ref([])
const loading = ref(false)

const list = computed(() => {
  if (!search.value.trim()) return templates.value
  const q = search.value.toLowerCase()
  return templates.value.filter(t => t.toLowerCase().includes(q))
})

async function load() {
  loading.value = true
  try { templates.value = await app.LicenseTemplateList() || [] }
  catch { templates.value = [] }
  finally { loading.value = false }
}

function startNew() {
  editing.value = { name: '', content: '', isNew: true }
  viewing.value = null
}

async function view(name) {
  editing.value = null
  try {
    const content = await app.LicenseTemplateView(name)
    viewing.value = { name, content }
  } catch { AMessage.error('读取失败') }
}

async function edit(name) {
  viewing.value = null
  try {
    const content = await app.LicenseTemplateView(name)
    editing.value = { name, origName: name, content, isNew: false }
  } catch { AMessage.error('读取失败') }
}

function confirmDelete(name) {
  Modal.confirm({
    title: '确认删除模板？',
    content: `将删除许可证模板「${name}」。`,
    okText: '删除', okType: 'danger', cancelText: '取消',
    async onOk() {
      try { await app.LicenseTemplateDelete(name); AMessage.success('已删除'); await load() }
      catch (e) { AMessage.error('删除失败: ' + String(e?.message || e)) }
    },
  })
}

// 仅名称必填；内容可为空（先建壳后补内容）。编辑态改名按重命名处理（删旧名，不留重复项）。
// 保存后留在编辑态（新建后可直接续填内容），不弹回列表。
async function save() {
  const t = editing.value
  if (!t?.name.trim()) { AMessage.warning('请填写模板名称'); return }
  const name = t.name.trim()
  try {
    await app.LicenseTemplateSave(name, t.content ?? '')
    if (!t.isNew && t.origName && t.origName !== name) {
      try { await app.LicenseTemplateDelete(t.origName) } catch {}
    }
    AMessage.success(!(t.content || '').trim() ? '已保存（内容为空，可稍后编辑）' : '已保存')
    editing.value = { name, origName: name, content: t.content ?? '', isNew: false }
    await load()
  } catch (e) { AMessage.error('保存失败: ' + String(e?.message || e)) }
}

onMounted(load)
</script>

<template>
  <div class="tp-pane">
    <!-- 左：列表 -->
    <div class="tp-side">
      <div class="tp-toolbar">
        <div class="tp-toolbar-row">
          <a-input v-model:value="search" placeholder="搜索许可证…" size="small" allow-clear class="tp-search" />
          <a-button size="small" type="primary" @click="startNew" title="新建模板"><PlusOutlined /></a-button>
        </div>
        <div class="tp-count">共 {{ templates.length }} 个模板（编辑后创建许可证时优先生效）</div>
      </div>
      <div class="tp-list">
        <div
          v-for="t in list"
          :key="t"
          class="tp-item"
          :class="{ active: viewing?.name === t || editing?.name === t }"
          @click="view(t)"
        >
          <span class="tp-item-name">{{ t }}</span>
          <div class="tp-item-acts" @click.stop>
            <a-button size="small" type="text" @click="edit(t)" title="编辑"><EditOutlined /></a-button>
            <a-button size="small" type="text" danger @click="confirmDelete(t)" title="删除"><DeleteOutlined /></a-button>
          </div>
        </div>
        <div v-if="!list.length && !loading" class="tp-empty-list">无匹配模板</div>
      </div>
    </div>

    <!-- 右：查看/编辑 -->
    <div class="tp-main">
      <template v-if="editing">
        <div class="tp-edit-head">
          <span class="tp-edit-tag">{{ editing.isNew ? '新建' : '编辑' }}</span>
          <a-input v-model:value="editing.name" placeholder="模板名称（如 MIT）" size="small" class="tp-name-input" />
          <a-button size="small" @click="editing = null">取消</a-button>
          <a-button size="small" type="primary" @click="save">保存</a-button>
        </div>
        <div class="tp-editor">
          <CodeViewer
            :code="editing.content"
            filename="LICENSE"
            editable
            class="tp-code"
            @update:modelValue="(v) => editing.content = v"
          />
        </div>
      </template>
      <template v-else-if="viewing">
        <div class="tp-view-head">{{ viewing.name }}<span class="tp-view-sub">许可证模板（可编辑）</span></div>
        <div class="tp-editor">
          <CodeViewer :code="viewing.content" filename="LICENSE" class="tp-code" />
        </div>
      </template>
      <div v-else class="tp-placeholder">从左侧选择模板查看，或点 + 新建</div>
    </div>
  </div>
</template>

<style scoped>
@import './pane.css';
</style>
