<script setup>
import { ref, computed, onMounted, onBeforeUnmount, h } from 'vue'
import { message as AMessage } from 'ant-design-vue'
import {
  GithubOutlined, SearchOutlined, CodeOutlined, StarOutlined,
} from '@ant-design/icons-vue'
import { useGitHubCache } from '../../../composables/useGitHub'
import { usePageFooter } from '../../../composables/usePageFooter'

/**
 * GitHub 搜索 — 代码搜索（当前仓库）+ 仓库搜索（全 GitHub）。
 */
const app = window.go?.main?.ByteGitApp
const { status, ensureStatusLoaded } = useGitHubCache()
const { setFooter, clearFooter } = usePageFooter()

const query = ref('')
const searchType = ref('code') // 'code' | 'repos'
const codeResults = ref([])
const repoResults = ref([])
const loading = ref(false)
const searched = ref(false)

const canUse = computed(() => status.value.authed)

async function doSearch() {
  const q = query.value.trim()
  if (!q || !canUse.value) return
  loading.value = true
  searched.value = true
  codeResults.value = []
  repoResults.value = []
  try {
    if (searchType.value === 'code') {
      codeResults.value = (await app.GhSearchCode(q)) || []
    } else {
      repoResults.value = (await app.GhSearchRepos(q)) || []
    }
  } catch (e) {
    AMessage.error('搜索失败: ' + String(e?.message || e))
  } finally {
    loading.value = false
  }
}

onMounted(async () => {
  await ensureStatusLoaded()
  if (status.value.authed) {
    setFooter(SearchFooter, () => ({ repo: status.value.ownerRepo || 'GitHub' }))
  }
})
onBeforeUnmount(() => clearFooter())

const SearchFooter = {
  props: ['repo'],
  render() {
    return h('div', { style: 'display:flex;align-items:center;gap:8px;width:100%;font-size:12px;color:var(--color-text-tertiary)' }, [
      h(SearchOutlined, { style: 'font-size:13px' }),
      h('span', { style: 'font-weight:500;color:var(--color-text)' }, '搜索'),
      h('span', this.repo),
    ])
  },
}

function fmtStars(n) {
  if (!n) return '0'
  if (n >= 1000) return (n / 1000).toFixed(1) + 'k'
  return String(n)
}

// GitHub API returns fragment with <mark> tags for matched text.
// Escape everything except <mark>/</mark> to prevent XSS while preserving highlights.
function cleanFragment(fragment) {
  if (!fragment) return ''
  // temporarily protect <mark> and </mark> tags
  let cleaned = fragment
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
  // restore <mark> and </mark> tags that were escaped
  cleaned = cleaned
    .replace(/&lt;mark&gt;/g, '<mark>')
    .replace(/&lt;\/mark&gt;/g, '</mark>')
  // limit to first ~15 lines
  const lines = cleaned.split('\n').slice(0, 15)
  return lines.join('\n')
}
</script>

<template>
  <div class="gs-view">
    <!-- search bar -->
    <div class="gs-bar">
      <a-radio-group v-model:value="searchType" size="small">
        <a-radio-button value="code"><CodeOutlined /> 代码</a-radio-button>
        <a-radio-button value="repos"><GithubOutlined /> 仓库</a-radio-button>
      </a-radio-group>
      <a-input-search
        v-model:value="query"
        :placeholder="searchType === 'code' ? '搜索当前仓库代码…' : '搜索 GitHub 仓库…'"
        enter-button
        size="small"
        style="flex: 1"
        @search="doSearch"
      />
    </div>

    <!-- scope hint for code search -->
    <div v-if="searchType === 'code' && status.ownerRepo" class="gs-scope">
      范围: <code>{{ status.ownerRepo }}</code>
    </div>

    <!-- results -->
    <div class="gs-results">
      <div v-if="loading" class="gs-center"><a-spin /></div>
      <div v-else-if="searched && searchType === 'code' && !codeResults.length" class="gs-empty">
        无匹配结果
      </div>
      <div v-else-if="searched && searchType === 'repos' && !repoResults.length" class="gs-empty">
        无匹配仓库
      </div>
      <div v-else-if="!searched" class="gs-empty">
        <SearchOutlined style="font-size:36px;opacity:0.2" />
        <div style="margin-top:8px">输入关键词开始搜索</div>
      </div>

      <!-- code results -->
      <template v-if="searchType === 'code' && codeResults.length">
        <div v-for="(r, i) in codeResults" :key="i" class="gs-code-item" @click="app?.OpenInBrowser(r.htmlUrl)">
          <div class="gs-code-head">
            <span class="gs-code-repo">{{ r.repo }}</span>
            <span class="gs-code-path">{{ r.path }}</span>
          </div>
          <div class="gs-code-name">{{ r.name }}</div>
          <pre v-if="r.fragment" class="gs-code-fragment" v-html="cleanFragment(r.fragment)"></pre>
        </div>
      </template>

      <!-- repo results -->
      <template v-if="searchType === 'repos' && repoResults.length">
        <div v-for="r in repoResults" :key="r.fullName" class="gs-repo-item" @click="app?.OpenInBrowser(r.htmlUrl)">
          <div class="gs-repo-head">
            <span class="gs-repo-name">{{ r.fullName }}</span>
            <span v-if="r.language" class="gs-repo-lang">{{ r.language }}</span>
            <span class="gs-repo-stars"><StarOutlined /> {{ fmtStars(r.stars) }}</span>
          </div>
          <div v-if="r.description" class="gs-repo-desc">{{ r.description }}</div>
        </div>
      </template>
    </div>
  </div>
</template>

<style scoped>
.gs-view { width: calc(100% + var(--spacing-lg) * 2); height: calc(100% + var(--spacing-lg) * 2); margin: calc(-1 * var(--spacing-lg)); display: flex; flex-direction: column; background: transparent; overflow: hidden; }

.gs-bar { display: flex; align-items: center; gap: 8px; height: 38px; padding: 0 var(--spacing-md); background: var(--color-surface); border-bottom: 1px solid var(--color-border); flex-shrink: 0; }
.gs-scope { font-size: 11px; color: var(--color-text-tertiary); margin-bottom: 8px; flex-shrink: 0; }
.gs-scope code { font-family: 'Cascadia Code', monospace; color: var(--color-primary); background: var(--color-primary-bg); padding: 0 4px; border-radius: 3px; }

.gs-results { flex: 1; overflow-y: auto; display: flex; flex-direction: column; gap: 6px; min-height: 0; padding: var(--spacing-md); }
.gs-center { display: flex; align-items: center; justify-content: center; height: 200px; }
.gs-empty { display: flex; flex-direction: column; align-items: center; justify-content: center; color: var(--color-text-tertiary); font-size: 13px; height: 200px; }

.gs-code-item {
  padding: 10px 14px; border-radius: var(--radius-sm);
  background: var(--color-surface); border: 1px solid var(--color-border);
  cursor: pointer; transition: all var(--transition-fast);
}
.gs-code-item:hover { border-color: var(--color-primary); }
.gs-code-head { display: flex; align-items: center; gap: 8px; }
.gs-code-repo { font-size: 12px; color: var(--color-primary); font-weight: 500; flex-shrink: 0; }
.gs-code-path { font-family: 'Cascadia Code', monospace; font-size: 11px; color: var(--color-text-secondary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.gs-code-name { font-size: 13px; color: var(--color-text); margin-top: 4px; }
.gs-code-fragment {
  font-family: 'Cascadia Code', monospace; font-size: 11px; line-height: 1.5;
  background: var(--color-background); padding: 8px 10px; border-radius: var(--radius-xs);
  margin: 6px 0 0; white-space: pre-wrap; word-break: break-word; overflow-x: auto;
  max-height: 240px; overflow-y: auto;
}
.gs-code-fragment mark {
  background: rgba(255, 213, 0, 0.3);
  color: inherit;
  border-radius: 2px;
  padding: 1px 2px;
  font-weight: 600;
}

.gs-repo-item {
  padding: 10px 14px; border-radius: var(--radius-sm);
  background: var(--color-surface); border: 1px solid var(--color-border);
  cursor: pointer; transition: all var(--transition-fast);
}
.gs-repo-item:hover { border-color: var(--color-primary); }
.gs-repo-head { display: flex; align-items: center; gap: 8px; }
.gs-repo-name { font-size: 14px; font-weight: 600; color: var(--color-text); }
.gs-repo-lang { font-size: 10px; padding: 1px 6px; border-radius: 3px; background: var(--color-info-bg); color: var(--color-info); font-weight: 500; }
.gs-repo-stars { font-size: 11px; color: var(--color-text-tertiary); margin-left: auto; display: inline-flex; align-items: center; gap: 3px; }
.gs-repo-desc { font-size: 12px; color: var(--color-text-secondary); margin-top: 4px; line-height: 1.5; }
</style>
