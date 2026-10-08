<script setup>
import { ref, onMounted } from 'vue'
import { useRepoData } from '../../composables/useRepoData'
import PanelState from '../common/PanelState.vue'
import MarkdownView from '../common/MarkdownView.vue'

// README is read directly (not via useRepoData) because it's a plain file,
// not a repo-analysed metric.
const app = window.go?.main?.ByteGitApp
const content = ref('')
const loading = ref(true)
const notFound = ref(false)

async function load() {
  loading.value = true
  if (!app?.RepoReadme) { loading.value = false; return }
  try {
    const res = await app.RepoReadme()
    notFound.value = res.notFound
    content.value = res.content || ''
  } catch (e) {
    notFound.value = true
  } finally {
    loading.value = false
  }
}
onMounted(load)
</script>

<template>
  <PanelState :loading="loading" :empty="notFound" empty-text="当前仓库未找到 README 文件">
    <MarkdownView :source="content" />
  </PanelState>
</template>
