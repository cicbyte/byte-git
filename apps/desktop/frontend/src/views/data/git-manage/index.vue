<script setup>
import { ref, computed, onMounted } from 'vue'
import { message as AMessage, Modal } from 'ant-design-vue'
import {
  CloudServerOutlined, BranchesOutlined, TagOutlined,
  PlusOutlined, ReloadOutlined, EditOutlined, DeleteOutlined,
  SwapOutlined, CheckOutlined, LinkOutlined, PushpinOutlined,
  CloudUploadOutlined,
} from '@ant-design/icons-vue'
import { useRepoStore } from '../../../stores/repo'
import { relTime } from '../../../composables/useRelativeTime'

const repo = useRepoStore()
const app = window.go?.main?.ByteGitApp

// ---- top-level tab ----
const tab = ref('remote') // remote | branch | tag

// ---- shared ----
const loading = ref(false)
async function reload() {
  loading.value = true
  try {
    if (tab.value === 'remote') await loadRemotes()
    else if (tab.value === 'branch') await loadBranches()
    else await loadTags()
  } finally {
    loading.value = false
  }
}
onMounted(reload)

// watch repo switch
import { watch } from 'vue'
watch(() => repo.repoEpoch, () => reload())

// ============================================================
// Remote
// ============================================================
const remotes = ref([])
async function loadRemotes() {
  if (!app?.RemoteList) return
  try { remotes.value = await app.RemoteList() || [] } catch (e) { AMessage.error('加载远程失败: ' + String(e?.message || e)) }
}

// add remote modal
const remoteAddOpen = ref(false)
const remoteAddForm = ref({ name: '', url: '' })
async function doAddRemote() {
  if (!remoteAddForm.value.name || !remoteAddForm.value.url) { AMessage.warning('名称和 URL 不能为空'); return }
  try {
    await app.RemoteAdd(remoteAddForm.value.name, remoteAddForm.value.url)
    AMessage.success('已添加远程')
    remoteAddOpen.value = false
    remoteAddForm.value = { name: '', url: '' }
    await loadRemotes()
  } catch (e) { AMessage.error('添加失败: ' + String(e?.message || e)) }
}

// edit url modal
const remoteEditOpen = ref(false)
const remoteEditTarget = ref(null)
const remoteEditUrl = ref('')
function openEditRemote(r) {
  remoteEditTarget.value = r
  remoteEditUrl.value = r.url
  remoteEditOpen.value = true
}
async function doEditRemote() {
  if (!remoteEditUrl.value) { AMessage.warning('URL 不能为空'); return }
  try {
    await app.RemoteSetURL(remoteEditTarget.value.name, remoteEditUrl.value)
    AMessage.success('已修改远程地址')
    remoteEditOpen.value = false
    await loadRemotes()
  } catch (e) { AMessage.error('修改失败: ' + String(e?.message || e)) }
}

function confirmRemoveRemote(r) {
  Modal.confirm({
    title: '确认删除远程？',
    content: `将删除远程「${r.name}」。本地分支不会删除，但跟踪关系可能失效。`,
    okText: '删除', okType: 'danger', cancelText: '取消',
    async onOk() {
      try { await app.RemoteRemove(r.name); AMessage.success('已删除'); await loadRemotes() }
      catch (e) { AMessage.error('删除失败: ' + String(e?.message || e)) }
    },
  })
}

// ============================================================
// Branch
// ============================================================
const branches = ref([])
async function loadBranches() {
  if (!app?.BranchList) return
  try { branches.value = await app.BranchList() || [] } catch (e) { AMessage.error('加载分支失败: ' + String(e?.message || e)) }
}
const localBranches = computed(() => branches.value.filter((b) => !b.isRemote))
const remoteBranches = computed(() => branches.value.filter((b) => b.isRemote))
const currentBranch = computed(() => localBranches.value.find((b) => b.current))

// create modal
const branchAddOpen = ref(false)
const branchAddName = ref('')
async function doAddBranch() {
  if (!branchAddName.value) { AMessage.warning('分支名不能为空'); return }
  try {
    await app.BranchCreate(branchAddName.value)
    AMessage.success('已创建分支')
    branchAddOpen.value = false
    branchAddName.value = ''
    await loadBranches()
  } catch (e) { AMessage.error('创建失败: ' + String(e?.message || e)) }
}

async function doSwitch(b) {
  try { await app.BranchSwitch(b.name); AMessage.success(`已切换到 ${b.name}`); await loadBranches() }
  catch (e) { AMessage.error('切换失败: ' + String(e?.message || e)) }
}

// rename modal
const renameOpen = ref(false)
const renameTarget = ref(null)
const renameNew = ref('')
function openRename(b) { renameTarget.value = b; renameNew.value = b.name; renameOpen.value = true }
async function doRename() {
  if (!renameNew.value) { AMessage.warning('新名称不能为空'); return }
  try {
    await app.BranchRename(renameTarget.value.name, renameNew.value)
    AMessage.success('已重命名')
    renameOpen.value = false
    await loadBranches()
  } catch (e) { AMessage.error('重命名失败: ' + String(e?.message || e)) }
}

// set upstream modal
const upstreamOpen = ref(false)
const upstreamTarget = ref(null)
const upstreamRemote = ref('')
function openUpstream(b) { upstreamTarget.value = b; upstreamRemote.value = b.upstream || (remotes.value[0]?.name ? remotes.value[0].name + '/' + b.name : ''); upstreamOpen.value = true }
async function doSetUpstream() {
  if (!upstreamRemote.value) { AMessage.warning('上游不能为空'); return }
  try {
    await app.BranchSetUpstream(upstreamTarget.value.name, upstreamRemote.value)
    AMessage.success('已设置上游')
    upstreamOpen.value = false
    await loadBranches()
  } catch (e) { AMessage.error('设置上游失败: ' + String(e?.message || e)) }
}

function confirmDeleteBranch(b, force) {
  Modal.confirm({
    title: force ? '强制删除分支？' : '删除分支？',
    content: force
      ? `强制删除「${b.name}」(即使未合并)，可能丢失未合并的提交。`
      : `删除「${b.name}」(仅允许已合并分支)。`,
    okText: '删除', okType: 'danger', cancelText: '取消',
    async onOk() {
      try {
        if (force) await app.BranchDeleteForce(b.name)
        else await app.BranchDelete(b.name)
        AMessage.success('已删除')
        await loadBranches()
      } catch (e) { AMessage.error('删除失败: ' + String(e?.message || e)) }
    },
  })
}

// ============================================================
// Tag
// ============================================================
const tags = ref([])
async function loadTags() {
  if (!app?.TagList) return
  try { tags.value = await app.TagList() || [] } catch (e) { AMessage.error('加载标签失败: ' + String(e?.message || e)) }
}

// create modal
const tagAddOpen = ref(false)
const tagAddForm = ref({ name: '', message: '' })
async function doAddTag() {
  if (!tagAddForm.value.name) { AMessage.warning('标签名不能为空'); return }
  try {
    await app.TagCreate(tagAddForm.value.name, tagAddForm.value.message)
    AMessage.success('已创建标签')
    tagAddOpen.value = false
    tagAddForm.value = { name: '', message: '' }
    await loadTags()
  } catch (e) { AMessage.error('创建失败: ' + String(e?.message || e)) }
}

function confirmDeleteTag(t) {
  Modal.confirm({
    title: '确认删除标签？',
    content: `将删除本地标签「${t.name}」(不影响远端)。`,
    okText: '删除', okType: 'danger', cancelText: '取消',
    async onOk() {
      try { await app.TagDelete(t.name); AMessage.success('已删除'); await loadTags() }
      catch (e) { AMessage.error('删除失败: ' + String(e?.message || e)) }
    },
  })
}

// push tag modal
const tagPushOpen = ref(false)
const tagPushRemote = ref('')
const tagPushNames = ref([])
function openTagPush(t) {
  tagPushNames.value = [t.name]
  tagPushRemote.value = remotes.value[0]?.name || 'origin'
  tagPushOpen.value = true
}
async function doTagPush() {
  if (!tagPushRemote.value || !tagPushNames.value.length) { AMessage.warning('远端和标签不能为空'); return }
  try {
    await app.TagPush(tagPushRemote.value, tagPushNames.value)
    AMessage.success('已推送标签')
    tagPushOpen.value = false
  } catch (e) { AMessage.error('推送失败: ' + String(e?.message || e)) }
}
</script>

<template>
  <div class="gm-view">
    <!-- top: tabs + actions -->
    <div class="gm-top">
      <a-segmented v-model:value="tab" :options="[
        { label: '远程仓库', value: 'remote' },
        { label: '分支', value: 'branch' },
        { label: '标签', value: 'tag' },
      ]" size="small" @change="reload" />
      <div class="gm-spacer"></div>
      <a-button size="small" @click="reload" :loading="loading"><template #icon><ReloadOutlined /></template>刷新</a-button>
      <a-button size="small" @click="tab === 'remote' ? (remoteAddOpen = true) : tab === 'branch' ? (branchAddOpen = true) : (tagAddOpen = true)">
        <template #icon><PlusOutlined /></template>{{ tab === 'remote' ? '添加远程' : tab === 'branch' ? '新建分支' : '创建标签' }}
      </a-button>
    </div>

    <a-spin :spinning="loading">
      <!-- ============ Remote ============ -->
      <div v-if="tab === 'remote'" class="gm-section">
        <div v-if="!remotes.length" class="gm-empty">暂无远程仓库</div>
        <div v-for="r in remotes" :key="r.name" class="gm-card">
          <div class="gm-card-head">
            <CloudServerOutlined class="gm-card-icon" />
            <span class="gm-card-title">{{ r.name }}</span>
            <a-tag v-for="t in r.types" :key="t" size="small">{{ t }}</a-tag>
            <div class="gm-spacer"></div>
            <a-button size="small" type="text" @click="openEditRemote(r)" title="修改地址"><EditOutlined /></a-button>
            <a-button size="small" type="text" @click="confirmRemoveRemote(r)" title="删除"><DeleteOutlined /></a-button>
          </div>
          <div class="gm-card-body">
            <div class="gm-url-row"><span class="gm-url-label">fetch</span><code class="gm-url" :title="r.url">{{ r.url }}</code></div>
            <div v-if="r.pushUrl && r.pushUrl !== r.url" class="gm-url-row"><span class="gm-url-label">push</span><code class="gm-url" :title="r.pushUrl">{{ r.pushUrl }}</code></div>
          </div>
        </div>
      </div>

      <!-- ============ Branch ============ -->
      <div v-else-if="tab === 'branch'" class="gm-section">
        <div class="gm-subhead">本地分支 ({{ localBranches.length }})</div>
        <div v-if="!localBranches.length" class="gm-empty">暂无本地分支</div>
        <div v-for="b in localBranches" :key="b.name" class="gm-card">
          <div class="gm-card-head">
            <BranchesOutlined class="gm-card-icon" :class="{ current: b.current }" />
            <span class="gm-card-title" :class="{ current: b.current }">{{ b.name }}</span>
            <a-tag v-if="b.current" color="green" size="small">当前</a-tag>
            <a-tag v-if="b.upstream" size="small" :color="b.gone ? 'red' : 'blue'">
              <LinkOutlined /> {{ b.upstream }}<template v-if="b.gone"> (gone)</template>
            </a-tag>
            <div class="gm-spacer"></div>
            <template v-if="!b.current">
              <a-button size="small" type="text" @click="doSwitch(b)" title="切换"><SwapOutlined /></a-button>
              <a-button size="small" type="text" @click="openRename(b)" title="重命名"><EditOutlined /></a-button>
              <a-button size="small" type="text" @click="openUpstream(b)" title="设置上游"><PushpinOutlined /></a-button>
              <a-button size="small" type="text" @click="confirmDeleteBranch(b, false)" title="删除(已合并)"><DeleteOutlined /></a-button>
            </template>
            <a-tag v-else color="default" size="small">当前所在</a-tag>
          </div>
        </div>

        <div class="gm-subhead" style="margin-top:16px">远程跟踪分支 ({{ remoteBranches.length }})</div>
        <div v-if="!remoteBranches.length" class="gm-empty">暂无远程分支</div>
        <div v-for="b in remoteBranches" :key="b.name" class="gm-card remote">
          <div class="gm-card-head">
            <BranchesOutlined class="gm-card-icon dim" />
            <span class="gm-card-title dim">{{ b.name }}</span>
          </div>
        </div>
      </div>

      <!-- ============ Tag ============ -->
      <div v-else class="gm-section">
        <div v-if="!tags.length" class="gm-empty">暂无标签</div>
        <div v-for="t in tags" :key="t.name" class="gm-card">
          <div class="gm-card-head">
            <TagOutlined class="gm-card-icon" />
            <span class="gm-card-title">{{ t.name }}</span>
            <a-tag size="small" :color="t.isAnnotated ? 'purple' : 'default'">{{ t.isAnnotated ? '附注' : '轻量' }}</a-tag>
            <code class="gm-commit">{{ t.commit }}</code>
            <span v-if="t.date" class="gm-date">{{ t.date }}</span>
            <div class="gm-spacer"></div>
            <a-button size="small" type="text" @click="openTagPush(t)" title="推送到远端"><CloudUploadOutlined /></a-button>
            <a-button size="small" type="text" @click="confirmDeleteTag(t)" title="删除"><DeleteOutlined /></a-button>
          </div>
          <div v-if="t.message" class="gm-card-body gm-tag-msg">{{ t.message }}</div>
        </div>
      </div>
    </a-spin>

    <!-- ===== Modals ===== -->
    <!-- add remote -->
    <a-modal v-model:open="remoteAddOpen" title="添加远程仓库" width="480px" :footer="null" destroy-on-close>
      <div class="gm-form">
        <div class="gm-form-row"><span class="gm-form-label">名称</span><a-input v-model:value="remoteAddForm.name" placeholder="如 origin" /></div>
        <div class="gm-form-row"><span class="gm-form-label">URL</span><a-input v-model:value="remoteAddForm.url" placeholder="https://github.com/user/repo.git" /></div>
        <div class="gm-form-foot"><a-button @click="remoteAddOpen = false">取消</a-button><a-button type="primary" @click="doAddRemote">添加</a-button></div>
      </div>
    </a-modal>

    <!-- edit remote url -->
    <a-modal v-model:open="remoteEditOpen" :title="`修改 ${remoteEditTarget?.name} 地址`" width="480px" :footer="null" destroy-on-close>
      <div class="gm-form">
        <div class="gm-form-row"><span class="gm-form-label">fetch URL</span><a-input v-model:value="remoteEditUrl" placeholder="https://..." /></div>
        <div class="gm-form-foot"><a-button @click="remoteEditOpen = false">取消</a-button><a-button type="primary" @click="doEditRemote">保存</a-button></div>
      </div>
    </a-modal>

    <!-- create branch -->
    <a-modal v-model:open="branchAddOpen" title="新建分支" width="420px" :footer="null" destroy-on-close>
      <div class="gm-form">
        <div class="gm-form-row"><span class="gm-form-label">分支名</span><a-input v-model:value="branchAddName" placeholder="如 feature/x" @press-enter="doAddBranch" /></div>
        <div class="gm-form-tip">基于当前 HEAD 创建</div>
        <div class="gm-form-foot"><a-button @click="branchAddOpen = false">取消</a-button><a-button type="primary" @click="doAddBranch">创建</a-button></div>
      </div>
    </a-modal>

    <!-- rename branch -->
    <a-modal v-model:open="renameOpen" :title="`重命名 ${renameTarget?.name}`" width="420px" :footer="null" destroy-on-close>
      <div class="gm-form">
        <div class="gm-form-row"><span class="gm-form-label">新名称</span><a-input v-model:value="renameNew" @press-enter="doRename" /></div>
        <div class="gm-form-foot"><a-button @click="renameOpen = false">取消</a-button><a-button type="primary" @click="doRename">重命名</a-button></div>
      </div>
    </a-modal>

    <!-- set upstream -->
    <a-modal v-model:open="upstreamOpen" :title="`设置上游 - ${upstreamTarget?.name}`" width="420px" :footer="null" destroy-on-close>
      <div class="gm-form">
        <div class="gm-form-row"><span class="gm-form-label">上游</span><a-input v-model:value="upstreamRemote" placeholder="如 origin/main" /></div>
        <div class="gm-form-tip">格式: remote/branch</div>
        <div class="gm-form-foot"><a-button @click="upstreamOpen = false">取消</a-button><a-button type="primary" @click="doSetUpstream">设置</a-button></div>
      </div>
    </a-modal>

    <!-- create tag -->
    <a-modal v-model:open="tagAddOpen" title="创建标签" width="420px" :footer="null" destroy-on-close>
      <div class="gm-form">
        <div class="gm-form-row"><span class="gm-form-label">标签名</span><a-input v-model:value="tagAddForm.name" placeholder="如 v1.0.0" /></div>
        <div class="gm-form-row"><span class="gm-form-label">说明</span><a-input v-model:value="tagAddForm.message" placeholder="留空=轻量标签，填写=附注标签" /></div>
        <div class="gm-form-tip">基于当前 HEAD 创建</div>
        <div class="gm-form-foot"><a-button @click="tagAddOpen = false">取消</a-button><a-button type="primary" @click="doAddTag">创建</a-button></div>
      </div>
    </a-modal>

    <!-- push tag -->
    <a-modal v-model:open="tagPushOpen" title="推送标签到远端" width="420px" :footer="null" destroy-on-close>
      <div class="gm-form">
        <div class="gm-form-row"><span class="gm-form-label">远端</span>
          <a-select v-model:value="tagPushRemote" style="width:100%">
            <a-select-option v-for="r in remotes" :key="r.name" :value="r.name">{{ r.name }}</a-select-option>
          </a-select>
        </div>
        <div class="gm-form-row"><span class="gm-form-label">标签</span>
          <a-select v-model:value="tagPushNames" mode="multiple" style="width:100%">
            <a-select-option v-for="t in tags" :key="t.name" :value="t.name">{{ t.name }}</a-select-option>
          </a-select>
        </div>
        <div class="gm-form-foot"><a-button @click="tagPushOpen = false">取消</a-button><a-button type="primary" @click="doTagPush">推送</a-button></div>
      </div>
    </a-modal>
  </div>
</template>

<style scoped>
.gm-view { width: 100%; height: 100%; display: flex; flex-direction: column; gap: var(--spacing-sm); min-height: 0; }
.gm-top { display: flex; align-items: center; gap: var(--spacing-sm); flex-shrink: 0; }
.gm-spacer { flex: 1; }

.gm-section { flex: 1; overflow-y: auto; padding-right: 4px; }
.gm-section::-webkit-scrollbar { width: 6px; }
.gm-section::-webkit-scrollbar-thumb { background: var(--color-border); border-radius: 3px; }

.gm-empty { padding: 40px; text-align: center; color: var(--color-text-tertiary); font-size: 13px; }
.gm-subhead { font-size: 11px; font-weight: 600; color: var(--color-text-tertiary); text-transform: uppercase; letter-spacing: 0.04em; margin-bottom: var(--spacing-xs); padding-left: 2px; }

.gm-card {
  background: var(--color-surface); border: 1px solid var(--color-border-light); border-radius: var(--radius-md);
  margin-bottom: var(--spacing-xs); transition: all var(--transition-fast);
}
.gm-card:hover { border-color: var(--color-border); }
.gm-card.remote { opacity: 0.7; }
.gm-card-head { display: flex; align-items: center; gap: 8px; padding: 10px 14px; }
.gm-card-body { padding: 0 14px 10px; }
.gm-card-icon { font-size: 15px; color: var(--color-text-tertiary); flex-shrink: 0; }
.gm-card-icon.current, .gm-card-icon.dim { color: var(--color-text-tertiary); }
.gm-card-icon.current { color: var(--color-success); }
.gm-card-title { font-size: 13px; font-weight: 600; color: var(--color-text); }
.gm-card-title.current { color: var(--color-primary); }
.gm-card-title.dim, .gm-card-icon.dim { color: var(--color-text-tertiary); font-weight: 400; }
.gm-url-row { display: flex; align-items: center; gap: 8px; }
.gm-url-label { font-size: 11px; color: var(--color-text-tertiary); width: 40px; flex-shrink: 0; }
.gm-url { font-family: 'Cascadia Code', monospace; font-size: 12px; color: var(--color-text-secondary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.gm-commit { font-family: 'Cascadia Code', monospace; font-size: 11px; color: var(--color-hash); }
.gm-date { font-size: 11px; color: var(--color-text-tertiary); }
.gm-tag-msg { font-size: 12px; color: var(--color-text-secondary); padding-top: 2px; }

/* forms in modals */
.gm-form { display: flex; flex-direction: column; gap: var(--spacing-md); padding: var(--spacing-sm) 0; }
.gm-form-row { display: flex; flex-direction: column; gap: 6px; }
.gm-form-label { font-size: 13px; color: var(--color-text-secondary); }
.gm-form-tip { font-size: 11px; color: var(--color-text-tertiary); }
.gm-form-foot { display: flex; justify-content: flex-end; gap: var(--spacing-sm); margin-top: var(--spacing-xs); }
</style>
