<script setup>
/**
 * Tab3 "解决方案" panel: three-step remediation for a leaked secret.
 *
 *  1. revoke / rotate the key (with platform hint link)
 *  2. remove the secret from the working tree file
 *  3. scrub the secret from the entire git history
 *
 * Receives the first active finding and the cached platform hints as props.
 * Emits `remove` and `scrub` actions back to the parent (which owns the app
 * calls). The auto-backup checkbox is two-way bound via v-model:scrub-auto-backup.
 */
import {
  ExclamationCircleOutlined, LinkOutlined, EditOutlined, FireOutlined,
} from '@ant-design/icons-vue'

defineProps({
  activeFile: { type: String, default: '' },
  findingCount: { type: Number, default: 0 },
  finding: { type: Object, default: null },
  hint: { type: Object, default: () => ({}) }, // { hint, url } for finding.RuleID
  scrubAutoBackup: { type: Boolean, default: true },
  resolveBusy: { type: Number, default: -1 }, // index being resolved; -1 = idle
})

defineEmits(['update:scrubAutoBackup', 'remove', 'scrub'])
</script>

<template>
  <div class="sv-tab-body sv-resolve-body">
    <!-- header: which finding we're resolving -->
    <div class="rv-header">
      <span class="rv-file">{{ activeFile }}</span>
      <span class="rv-sub">{{ findingCount }} 处泄露</span>
    </div>

    <!-- ① revoke / rotate — the must-do step -->
    <div class="rv-step rv-step-revoke">
      <div class="rv-step-head">
        <span class="rv-step-num">1</span>
        <span class="rv-step-title"><ExclamationCircleOutlined /> 立即吊销/轮换此密钥</span>
        <a-tag color="red" class="rv-must">必做</a-tag>
      </div>
      <div class="rv-step-body">
        <p class="rv-explain">密钥一旦进入 git 历史，应视为已泄露。即使从代码中移除，攻击者仍可从历史中获取。请先到对应平台吊销该密钥。</p>
        <div class="rv-hint-box" v-if="finding">
          <span class="rv-hint-label">指引:</span>
          <span class="rv-hint-text">{{ hint?.hint || '联系对应服务提供商吊销该密钥' }}</span>
          <a-button v-if="hint?.url"
            type="link" size="small"
            :href="hint.url"
            target="_blank" class="rv-open-btn">
            <LinkOutlined /> 打开控制台
          </a-button>
        </div>
      </div>
    </div>

    <!-- ② remove from working tree -->
    <div class="rv-step rv-step-remove">
      <div class="rv-step-head">
        <span class="rv-step-num">2</span>
        <span class="rv-step-title"><EditOutlined /> 从当前文件移除密钥</span>
        <a-tag color="orange">中风险</a-tag>
      </div>
      <div class="rv-step-body">
        <p class="rv-explain">将工作区文件中的密钥替换为 <code>REMOVED_SECRET</code> 占位符并暂存（git add）。</p>
        <p class="rv-warn">⚠️ 只修复当前版本，密钥仍保留在 git 历史中。需配合第 3 步彻底清除。</p>
        <a-button v-if="finding"
          type="primary" size="small"
          :loading="resolveBusy === 0"
          @click="$emit('remove', finding, 0)">
          从文件移除
        </a-button>
      </div>
    </div>

    <!-- ③ scrub history -->
    <div class="rv-step rv-step-scrub">
      <div class="rv-step-head">
        <span class="rv-step-num">3</span>
        <span class="rv-step-title"><FireOutlined /> 从 Git 历史彻底抹除</span>
        <a-tag color="red">高危</a-tag>
      </div>
      <div class="rv-step-body">
        <p class="rv-explain">使用 <code>git-filter-repo</code> 把所有历史 commit 中的密钥替换为 <code>REMOVED_SECRET</code>。</p>
        <ul class="rv-warn-list">
          <li>改写所有受影响 commit 的 hash</li>
          <li>filter-repo 会移除 origin remote（需重新 <code>git remote add origin</code>）</li>
          <li>已推送的仓库需 <code>git push --force</code>，协作者须重新 clone</li>
        </ul>
        <div class="rv-backup-opt">
          <a-checkbox :checked="scrubAutoBackup" @change="$emit('update:scrubAutoBackup', $event.target.checked)">抹除前先整库克隆备份</a-checkbox>
        </div>
        <a-button v-if="finding"
          danger size="small"
          :loading="resolveBusy === 0"
          @click="$emit('scrub', finding, 0)">
          从历史抹除
        </a-button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.sv-tab-body { flex: 1; min-height: 0; overflow-y: auto; padding: var(--spacing-sm) var(--spacing-md); }
.sv-resolve-body { padding: var(--spacing-md) var(--spacing-md); }
.rv-header { margin-bottom: var(--spacing-md); padding-bottom: var(--spacing-sm); border-bottom: 1px solid var(--color-border-light); }
.rv-file { font-family: 'Cascadia Code', monospace; font-size: 13px; color: var(--color-text); font-weight: 600; }
.rv-sub { margin-left: 8px; font-size: 12px; color: var(--color-text-tertiary); }

.rv-step { margin-bottom: var(--spacing-md); border: 1px solid var(--color-border); border-radius: var(--radius-md); overflow: hidden; background: var(--color-surface); }
.rv-step-revoke { border-color: var(--color-error); }
.rv-step-remove { border-color: var(--color-warning); }
.rv-step-scrub { border-color: var(--color-error); }

.rv-step-head { display: flex; align-items: center; gap: 10px; padding: 10px 14px; font-weight: 600; font-size: 13px; }
.rv-step-revoke .rv-step-head { background: var(--color-error-bg); color: var(--color-error); }
.rv-step-remove .rv-step-head { background: var(--color-warning-bg); color: var(--color-warning); }
.rv-step-scrub .rv-step-head { background: var(--color-error-bg); color: var(--color-error); }

.rv-step-num { display: inline-flex; align-items: center; justify-content: center; width: 22px; height: 22px; border-radius: 50%; font-size: 12px; font-weight: 700; flex-shrink: 0; }
.rv-step-revoke .rv-step-num { background: var(--color-error); color: #fff; }
.rv-step-remove .rv-step-num { background: var(--color-warning); color: #fff; }
.rv-step-scrub .rv-step-num { background: var(--color-error); color: #fff; }
.rv-step-title { flex: 1; display: flex; align-items: center; gap: 6px; }
.rv-must { margin-left: 0; }

.rv-step-body { padding: 12px 14px; }
.rv-explain { font-size: 12px; color: var(--color-text-secondary); line-height: 1.6; margin: 0 0 8px 0; }
.rv-explain code, .rv-warn code, .rv-warn-list code, .rv-note code { font-family: 'Cascadia Code', monospace; font-size: 11px; color: var(--color-primary); background: var(--color-background); padding: 1px 4px; border-radius: 3px; }
.rv-warn { font-size: 12px; color: var(--color-warning); margin: 0 0 10px 0; }
.rv-warn-list { font-size: 12px; color: var(--color-text-secondary); line-height: 1.8; margin: 0 0 10px 0; padding-left: 18px; }
.rv-note { font-size: 12px; color: var(--color-success); margin: 0 0 10px 0; }

.rv-hint-box { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; padding: 8px 10px; background: var(--color-background); border-radius: var(--radius-sm); margin-bottom: 10px; }
.rv-hint-label { font-size: 11px; color: var(--color-text-tertiary); flex-shrink: 0; }
.rv-hint-text { font-size: 12px; color: var(--color-text); flex: 1; }
.rv-open-btn { padding: 0 4px !important; height: auto !important; font-size: 12px; }

.rv-backup-opt { margin-bottom: 10px; }
</style>
