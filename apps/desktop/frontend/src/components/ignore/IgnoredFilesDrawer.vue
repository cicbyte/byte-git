<script setup>
/**
 * 被忽略文件抽屉 — 防误忽略视图。
 * 列出仓库中被忽略的文件/目录及命中的规则（模式 + 来源行），
 * 疑似误忽略项（源码类文件被忽略、或非依赖目录内含源码文件）优先标出。
 * 动作：定位规则（根 .gitignore）、注释掉规则、复制路径。
 */
import { ref, computed, watch } from 'vue'
import { message as AMessage, Modal } from 'ant-design-vue'
import {
  FolderOutlined, FileOutlined, WarningOutlined,
  AimOutlined, CommentOutlined, CopyOutlined, ReloadOutlined,
} from '@ant-design/icons-vue'

const app = window.go?.main?.ByteGitApp

const props = defineProps({ open: Boolean })
const emit = defineEmits(['update:open', 'locate', 'commentRule'])

const entries = ref([])
const truncated = ref(false)
const loading = ref(false)
const search = ref('')
const onlySuspicious = ref(false)
const displayLimit = ref(300)

async function load() {
  if (!app?.IgnoreIgnoredList) return
  loading.value = true
  try {
    const res = await app.IgnoreIgnoredList()
    entries.value = res.entries || []
    truncated.value = !!res.truncated
    displayLimit.value = 300
  } catch (e) {
    AMessage.error('加载被忽略文件失败: ' + String(e?.message || e))
    entries.value = []
  } finally {
    loading.value = false
  }
}

watch(() => props.open, (v) => { if (v) load() })

const suspiciousCount = computed(() => entries.value.filter(e => e.suspicious).length)

const filtered = computed(() => {
  let list = entries.value
  if (onlySuspicious.value) list = list.filter(e => e.suspicious)
  const q = search.value.trim().toLowerCase()
  if (q) {
    list = list.filter(e =>
      e.path.toLowerCase().includes(q) || (e.rule || '').toLowerCase().includes(q))
  }
  return list
})

const visible = computed(() => filtered.value.slice(0, displayLimit.value))

// 根 .gitignore 的规则才可在编辑器中定位/注释（嵌套或全局来源只读展示）
function isRootRule(e) {
  return e.line > 0 && (e.source || '').startsWith('.gitignore:')
}

function locate(e) {
  emit('locate', e)
}

function copyPath(e) {
  navigator.clipboard?.writeText(e.path)
    .then(() => AMessage.success('已复制路径'))
    .catch(() => AMessage.error('复制失败'))
}

function commentRule(e) {
  Modal.confirm({
    title: '注释掉这条规则？',
    content: `将在 .gitignore 第 ${e.line} 行行首添加 "# "，让「${e.rule}」不再生效（可随时手动恢复）。`,
    okText: '注释',
    cancelText: '取消',
    onOk() { emit('commentRule', e) },
  })
}
</script>

<template>
  <a-drawer
    :open="open"
    @update:open="(v) => emit('update:open', v)"
    title="被忽略文件"
    width="640px"
    :body-style="{ padding: '12px 16px', display: 'flex', flexDirection: 'column' }"
  >
    <template #extra>
      <a-button size="small" type="text" @click="load" :loading="loading"><ReloadOutlined /></a-button>
    </template>

    <!-- toolbar -->
    <div class="ig-toolbar">
      <span class="ig-stat">共 {{ entries.length }} 项</span>
      <span v-if="suspiciousCount" class="ig-stat warn"><WarningOutlined /> {{ suspiciousCount }} 项疑似误忽略</span>
      <span v-if="truncated" class="ig-stat warn">条目过多，仅显示前 2000</span>
      <div class="ig-spacer" />
      <a-radio-group v-model:value="onlySuspicious" size="small" button-style="solid">
        <a-radio-button :value="false">全部</a-radio-button>
        <a-radio-button :value="true">疑似误忽略</a-radio-button>
      </a-radio-group>
      <a-input v-model:value="search" placeholder="搜索路径 / 规则…" size="small" allow-clear class="ig-search" />
    </div>

    <!-- list -->
    <div class="ig-list">
      <div v-if="loading" class="ig-center"><a-spin tip="扫描被忽略文件…" /></div>
      <div v-else-if="!filtered.length" class="ig-center">
        <a-empty :description="entries.length ? '没有符合筛选的条目' : '没有被忽略的文件'" :image-style="{ height: '48px' }" />
      </div>
      <template v-else>
        <div
          v-for="e in visible"
          :key="e.path"
          class="ig-row"
          :class="{ warn: e.suspicious }"
        >
          <component
            :is="e.isDir ? FolderOutlined : FileOutlined"
            class="ig-row-icon"
            :class="{ warn: e.suspicious }"
          />
          <div class="ig-row-main">
            <div class="ig-row-path">
              <span class="ig-path-text">{{ e.path }}</span>
              <span v-if="e.isDir" class="ig-tag dir">目录</span>
              <span v-if="e.suspicious" class="ig-tag warn"><WarningOutlined /> 疑似误忽略</span>
            </div>
            <div class="ig-row-rule">
              <template v-if="e.rule">
                <span class="ig-rule">规则 {{ e.rule }}</span>
                <span class="ig-source">{{ e.source }}</span>
              </template>
              <span v-else class="ig-source">未匹配到具体规则</span>
            </div>
          </div>
          <div class="ig-row-acts">
            <a-tooltip v-if="isRootRule(e)" title="在 .gitignore 中定位该规则">
              <a-button size="small" type="text" @click="locate(e)"><AimOutlined /></a-button>
            </a-tooltip>
            <a-tooltip v-if="isRootRule(e)" title="注释掉该规则（使其不再生效）">
              <a-button size="small" type="text" @click="commentRule(e)"><CommentOutlined /></a-button>
            </a-tooltip>
            <a-tooltip title="复制路径">
              <a-button size="small" type="text" @click="copyPath(e)"><CopyOutlined /></a-button>
            </a-tooltip>
          </div>
        </div>
        <div v-if="filtered.length > visible.length" class="ig-more">
          <a-button size="small" @click="displayLimit += 300">
            加载更多（已显示 {{ visible.length }} / {{ filtered.length }}）
          </a-button>
        </div>
      </template>
    </div>

    <div class="ig-foot">
      「疑似误忽略」= 源码类文件被忽略，或非依赖目录内含源码文件（如 lib/ 规则吞掉前端源码）。
    </div>
  </a-drawer>
</template>

<style scoped>
.ig-toolbar { display: flex; align-items: center; gap: 8px; margin-bottom: 8px; flex-shrink: 0; }
.ig-stat { font-size: 11px; color: var(--color-text-tertiary); }
.ig-stat.warn { color: var(--color-warning); }
.ig-spacer { flex: 1; }
.ig-search { width: 180px; }

.ig-list { flex: 1; min-height: 0; overflow-y: auto; border-top: 1px solid var(--color-border-light); }
.ig-center { padding: 40px 0; display: flex; justify-content: center; }

.ig-row { display: flex; align-items: center; gap: 10px; padding: 7px 8px; border-bottom: 1px solid var(--color-border-light); }
.ig-row.warn { background: rgba(var(--color-primary-rgb), 0.04); }
.ig-row-icon { font-size: 15px; color: var(--color-text-tertiary); flex-shrink: 0; }
.ig-row-icon.warn { color: var(--color-warning); }
.ig-row-main { flex: 1; min-width: 0; }
.ig-row-path { display: flex; align-items: center; gap: 6px; }
.ig-path-text { font-size: 12px; color: var(--color-text); word-break: break-all; }
.ig-tag { font-size: 10px; padding: 0 5px; border-radius: 3px; line-height: 16px; flex-shrink: 0; }
.ig-tag.dir { color: var(--color-text-tertiary); border: 1px solid var(--color-border); }
.ig-tag.warn { color: var(--color-warning); border: 1px solid var(--color-warning); }
.ig-row-rule { display: flex; align-items: center; gap: 8px; margin-top: 2px; }
.ig-rule { font-size: 11px; color: var(--color-text-secondary); font-family: var(--font-mono, monospace); }
.ig-source { font-size: 10px; color: var(--color-text-tertiary); }
.ig-row-acts { display: flex; gap: 2px; opacity: 0; flex-shrink: 0; }
.ig-row:hover .ig-row-acts { opacity: 1; }

.ig-more { display: flex; justify-content: center; padding: 10px 0; }

.ig-foot { flex-shrink: 0; padding-top: 8px; font-size: 10px; color: var(--color-text-tertiary); opacity: 0.7; border-top: 1px solid var(--color-border-light); }
</style>
