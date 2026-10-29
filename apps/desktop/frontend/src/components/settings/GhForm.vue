<script setup>
import { ref, onMounted } from 'vue'
import {
  CheckCircleOutlined, CloseCircleOutlined, ReloadOutlined,
  GithubOutlined, ExclamationCircleOutlined,
} from '@ant-design/icons-vue'
import { useSettings, SETTING_KEYS, useFeatureFlags } from '@/composables/useSettings'
import { useGitHubCache } from '@/composables/useGitHub'

/**
 * GitHub 配置 — Token 认证 + 远端检测。
 * 认证状态通过 useGitHubCache 缓存，与 GitHubView 共享。
 */
const app = window.go?.main?.ByteGitApp
const { settings, queueSave } = useSettings()
const { githubEnabled } = useFeatureFlags()
const { status, ensureStatusLoaded, refreshStatus, invalidateStatus } = useGitHubCache()

const checking = ref(false)

function onToggle(v) {
  // update local state immediately so the switch flips without waiting for
  // the debounced save round-trip (a-switch uses one-way :checked binding)
  settings.featuresGitHub = v
  queueSave(SETTING_KEYS.featuresGitHub, v)
}

async function check() {
  invalidateStatus()
  checking.value = true
  await refreshStatus()
  checking.value = false
}

function onTokenChange(v) {
  queueSave(SETTING_KEYS.githubToken, v)
  invalidateStatus()
  setTimeout(() => refreshStatus(), 800)
}

onMounted(() => { ensureStatusLoaded() })
</script>

<template>
  <div class="gf-form">
    <!-- feature toggle -->
    <div class="setting-group feature-toggle">
      <div class="ft-row">
        <div class="ft-label">
          <div class="ft-title">启用 GitHub 功能</div>
          <div class="ft-desc">开启后在侧边栏显示 GitHub 菜单组（PR / Issues / Actions / Releases / 搜索 / Trending）</div>
        </div>
        <a-switch :checked="githubEnabled" @change="onToggle" />
      </div>
    </div>

    <div :class="{ 'ft-disabled': !githubEnabled }">
    <!-- Token configuration -->
    <div class="setting-group">
      <div class="group-title"><GithubOutlined /> GitHub Token</div>
      <div class="setting-row">
        <div class="row-label">
          <div class="row-title">Access Token</div>
          <div class="row-help">用于 PR 管理、Release 下载、API 访问。在 GitHub Settings → Developer settings → Personal access tokens 创建，需 repo 权限。</div>
        </div>
        <a-input-password
          :value="settings.githubToken"
          placeholder="ghp_..."
          style="width: 380px"
          @change="(e) => onTokenChange(e.target.value)"
        />
      </div>
    </div>

    <!-- status card -->
    <div class="setting-group">
      <div class="group-title">连接状态</div>

      <div class="gf-status-bar">
        <!-- token set -->
        <div class="gf-status-item">
          <CheckCircleOutlined v-if="status.tokenSet" class="gf-ok" />
          <CloseCircleOutlined v-else class="gf-err" />
          <span class="gf-status-label">Token</span>
          <span class="gf-status-value">{{ status.tokenSet ? '已配置' : '未配置' }}</span>
        </div>
        <!-- authed -->
        <div class="gf-status-item">
          <CheckCircleOutlined v-if="status.authed" class="gf-ok" />
          <CloseCircleOutlined v-else class="gf-err" />
          <span class="gf-status-label">认证</span>
          <span class="gf-status-value">
            {{ status.authed ? `已认证 (${status.user})` : '未认证' }}
          </span>
        </div>
        <!-- remote -->
        <div class="gf-status-item">
          <CheckCircleOutlined v-if="status.remoteIsGh" class="gf-ok" />
          <CloseCircleOutlined v-else class="gf-err" />
          <span class="gf-status-label">远端</span>
          <span class="gf-status-value">
            {{ status.remoteIsGh ? status.ownerRepo : '非 GitHub 仓库' }}
          </span>
        </div>
        <a-button size="small" type="text" :loading="checking" @click="check" title="重新检测">
          <template #icon><ReloadOutlined /></template>
        </a-button>
      </div>

      <!-- not configured -->
      <a-alert v-if="!status.tokenSet" type="info" show-icon style="margin-top: 12px">
        <template #message>配置 Token 后即可使用 GitHub PR 管理等功能</template>
        <template #description>
          <div>1. 打开 <a href="https://github.com/settings/tokens" target="_blank">GitHub Settings → Tokens</a></div>
          <div>2. 创建 Token，勾选 <code>repo</code> 权限</div>
          <div>3. 将 Token 粘贴到上方输入框</div>
        </template>
      </a-alert>

      <!-- token invalid -->
      <a-alert v-if="status.tokenSet && !status.authed" type="error" show-icon style="margin-top: 12px"
        message="Token 无效或已过期"
        description="请检查 Token 是否正确，是否已过期或被撤销。重新创建后粘贴到上方。" />

      <!-- ready -->
      <div v-if="status.authed" class="gf-ready" style="margin-top: 12px">
        <CheckCircleOutlined class="gf-ok" />
        <span>{{ status.remoteIsGh ? `已就绪。当前仓库 ${status.ownerRepo} 的 PR 管理可用。` : '已认证，但当前仓库不是 GitHub 仓库。' }}</span>
      </div>
    </div>
    </div><!-- /ft-disabled wrapper -->
  </div>
</template>

<style scoped>
.gf-form { flex: 1; overflow-y: auto; padding: 20px 24px; background: var(--color-background); }

/* feature toggle */
.feature-toggle { border-left: 3px solid var(--color-primary); }
.ft-row { display: flex; align-items: center; justify-content: space-between; gap: 14px; }
.ft-label { flex: 1; }
.ft-title { font-size: 14px; font-weight: 600; color: var(--color-text); }
.ft-desc { font-size: 12px; color: var(--color-text-tertiary); margin-top: 3px; }
.ft-disabled { opacity: 0.4; pointer-events: none; }

.setting-group {
  margin-bottom: var(--spacing-md);
  border-radius: var(--radius-md);
  border: 1px solid var(--color-border);
  padding: 16px 18px;
  background: var(--color-surface);
  box-shadow: var(--shadow-xs);
}
.group-title {
  display: flex; align-items: center; gap: 8px;
  font-size: 14px; font-weight: 600; color: var(--color-text);
  margin-bottom: 12px;
}

.gf-status-bar {
  display: flex; align-items: center; gap: 16px; flex-wrap: wrap;
  padding: 10px 0;
}
.gf-status-item { display: flex; align-items: center; gap: 6px; font-size: 13px; }
.gf-ok { color: var(--color-success); flex-shrink: 0; }
.gf-err { color: var(--color-error); flex-shrink: 0; }
.gf-status-label { color: var(--color-text-tertiary); font-weight: 600; min-width: 32px; }
.gf-status-value { color: var(--color-text); }

.setting-row {
  display: flex; justify-content: space-between; align-items: center;
  min-height: 36px; padding: 6px 0;
}
.row-label { display: flex; flex-direction: column; }
.row-title { font-size: 14px; color: var(--color-text); }
.row-help { font-size: 12px; color: var(--color-text-tertiary); margin-top: 4px; }

.gf-ready { display: flex; align-items: center; gap: 8px; font-size: 13px; color: var(--color-text-secondary); }
</style>
