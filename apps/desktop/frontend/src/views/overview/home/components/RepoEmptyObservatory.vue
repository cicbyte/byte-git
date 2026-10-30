<script setup>
/**
 * 无仓库首屏空状态 ——「夜观图录」观测图版。
 *
 * 方向稿：design-demos/dir-1.html（巨号衬线标题 + 轨道线图 + 等宽诚实注记 +
 * 「观测未始」印章 + 右侧动作条列）。commit 历史天然是轨道：仓库是中心天体，
 * commit 是待入轨的卫星点，空 = 观测尚未开始。
 *
 * 主题适配：纸面/墨色/靛蓝全部走主题令牌（surface / text / primary），
 * 深色主题下是深色纸、浅色主题下是米白纸，不硬编码色值。
 * 动作只用真实能力：浏览文件夹 / 粘贴路径回车切换 / 最近打开 / 重新检测。
 */
import { ref } from 'vue'
import { message as AMessage } from 'ant-design-vue'
import { FolderOpenOutlined, ReloadOutlined } from '@ant-design/icons-vue'
import { useRepoStore } from '@/stores/repo'

const repo = useRepoStore()

const picking = ref(false)
const switching = ref(false)
const pastePath = ref('')

async function pick() {
  if (picking.value) return
  picking.value = true
  try {
    const dir = await repo.pickFolder()
    if (dir) await repo.switchTo(dir)
  } finally {
    picking.value = false
  }
}

async function switchPaste() {
  const p = pastePath.value.trim()
  if (!p || switching.value) return
  switching.value = true
  try {
    const ok = await repo.switchTo(p)
    if (ok) {
      pastePath.value = ''
    } else {
      AMessage.error('路径无效或不是 Git 仓库')
    }
  } finally {
    switching.value = false
  }
}

async function redetect() {
  await repo.detectCurrent()
  AMessage.info('已重新检测当前目录')
}

function openModal() {
  window.dispatchEvent(new CustomEvent('open-repo-modal'))
}
</script>

<template>
  <div class="obs">
    <!-- 图版（纸面） -->
    <section class="obs-plate">
      <i class="obs-crop tl" /><i class="obs-crop tr" /><i class="obs-crop bl" /><i class="obs-crop br" />
      <div class="obs-inner">
        <div class="obs-crumb">观测图录 · 空状态 / 图版一</div>
        <h1 class="obs-title">第一轨道</h1>
        <p class="obs-sub">The First Orbit — awaiting the first commit</p>
        <p class="obs-desc">还没有打开仓库。选择一个 Git 仓库，让它的提交开始入轨。</p>

        <!-- 轨道示意（主题令牌着色） -->
        <div class="obs-diagram">
          <svg viewBox="0 0 560 300" preserveAspectRatio="xMidYMid meet">
            <!-- 角度刻度 -->
            <text class="obs-tick" x="28" y="162">180°</text>
            <text class="obs-tick" x="512" y="162">0°</text>
            <text class="obs-tick" x="272" y="42">90°</text>
            <text class="obs-tick" x="268" y="292">270°</text>
            <!-- 外轨（实线）与内轨（虚线） -->
            <ellipse class="obs-orbit" cx="280" cy="158" rx="248" ry="104" />
            <ellipse class="obs-orbit obs-orbit-dash" cx="280" cy="158" rx="150" ry="62" />
            <!-- 观测线 -->
            <line class="obs-sight" x1="60" y1="52" x2="252" y2="138" />
            <!-- 中心天体：仓库 -->
            <circle class="obs-planet-halo" cx="280" cy="158" r="34" />
            <circle class="obs-planet" cx="280" cy="158" r="24" />
            <text class="obs-planet-label" x="280" y="216">仓库</text>
            <!-- 已入轨的卫星点（历史 commit 的位置，此刻为空） -->
            <circle class="obs-sat" cx="80" cy="112" r="3.5" />
            <!-- 待入轨的空心节点：第一个 commit -->
            <circle class="obs-node" cx="492" cy="224" r="9" />
            <circle class="obs-node-core" cx="492" cy="224" r="2.6" />
            <line class="obs-callout" x1="484" y1="216" x2="420" y2="168" />
            <text class="obs-callout-text" x="300" y="158">第一个 commit 尚未发生</text>
          </svg>
        </div>

        <div class="obs-fig">FIG. 01 — 轨道示意，非比例 · 仓库 0 · 提交 0 · 观测未始</div>
      </div>
      <!-- 印章 -->
      <div class="obs-stamp" aria-hidden="true">
        <span>观测未始</span>
        <span class="obs-stamp-en">NOT STARTED</span>
      </div>
    </section>

    <!-- 右侧动作条列 -->
    <aside class="obs-side">
      <div class="obs-side-label">下一步</div>
      <a-button type="primary" block :loading="picking" @click="pick">
        <FolderOpenOutlined /> 浏览文件夹
      </a-button>
      <div class="obs-paste">
        <a-input
          v-model:value="pastePath"
          placeholder="粘贴仓库路径，回车切换"
          :disabled="switching"
          @press-enter="switchPaste"
        />
        <a-button size="small" :loading="switching" @click="switchPaste">切换</a-button>
      </div>

      <div class="obs-rule" />

      <div class="obs-side-label">最近打开</div>
      <div v-if="repo.recent.length" class="obs-recent">
        <button
          v-for="r in repo.recent.slice(0, 5)"
          :key="r.path"
          class="obs-recent-item"
          :title="r.path"
          @click="repo.switchTo(r.path)"
        >
          <span class="obs-recent-name">{{ r.name }}</span>
          <span class="obs-recent-path">{{ r.path }}</span>
        </button>
      </div>
      <div v-else class="obs-recent-empty">
        <svg class="obs-recent-glyph" viewBox="0 0 40 26" aria-hidden="true">
          <ellipse class="obs-orbit obs-orbit-dash" cx="20" cy="13" rx="16" ry="8" />
          <circle class="obs-node" cx="33" cy="17" r="4" />
        </svg>
        <span>暂无最近打开的仓库</span>
      </div>

      <div class="obs-rule" />

      <div class="obs-foot-links">
        <button class="obs-link" @click="redetect"><ReloadOutlined /> 重新检测</button>
        <button class="obs-link" @click="openModal">管理最近列表</button>
      </div>
    </aside>
  </div>
</template>

<style scoped>
.obs {
  display: flex;
  gap: var(--spacing-lg, 16px);
  align-items: stretch;
  height: calc(100vh - 200px);
  min-height: 480px;
}

/* ---- 图版纸面：随主题（暗=深色纸 / 亮=米白纸）---- */
.obs-plate {
  position: relative;
  flex: 2.2;
  min-width: 0;
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md, 8px);
  overflow: hidden;
}
.obs-inner {
  position: absolute;
  inset: 10px;
  border: 1px solid var(--color-border-light);
  border-radius: var(--radius-sm, 6px);
  padding: 26px 34px 20px;
  display: flex;
  flex-direction: column;
}
/* 角规线（图版四角刻度） */
.obs-crop { position: absolute; width: 10px; height: 10px; pointer-events: none; }
.obs-crop::before, .obs-crop::after { content: ''; position: absolute; background: var(--color-text-disabled); }
.obs-crop::before { width: 10px; height: 1px; }
.obs-crop::after { width: 1px; height: 10px; }
.obs-crop.tl { top: 3px; left: 3px; }
.obs-crop.tr { top: 3px; right: 3px; } .obs-crop.tr::before { right: 0; } .obs-crop.tr::after { right: 0; }
.obs-crop.bl { bottom: 3px; left: 3px; } .obs-crop.bl::before { bottom: 0; } .obs-crop.bl::after { bottom: 0; }
.obs-crop.br { bottom: 3px; right: 3px; } .obs-crop.br::before { right: 0; bottom: 0; } .obs-crop.br::after { right: 0; bottom: 0; }

.obs-crumb {
  font-family: var(--font-mono, 'JetBrains Mono', Consolas, monospace);
  font-size: 12px;
  letter-spacing: 2px;
  color: var(--color-text-tertiary);
}
.obs-title {
  font-family: 'Noto Serif SC', 'Source Han Serif SC', 'Songti SC', 'SimSun', serif;
  font-size: 44px;
  font-weight: 900;
  line-height: 1.1;
  color: var(--color-text);
  margin-top: 10px;
}
.obs-sub {
  font-family: 'Noto Serif SC', 'Source Han Serif SC', 'Songti SC', 'SimSun', serif;
  font-style: italic;
  font-size: 16px;
  color: var(--color-primary);
  margin-top: 6px;
}
.obs-desc {
  font-size: 14px;
  color: var(--color-text-secondary);
  margin-top: 10px;
}

/* 轨道图 */
.obs-diagram { flex: 1; min-height: 0; margin-top: 6px; }
.obs-diagram svg { width: 100%; height: 100%; }
.obs-orbit { fill: none; stroke: var(--color-text-tertiary); stroke-opacity: 0.45; stroke-width: 1; }
.obs-orbit-dash { stroke-dasharray: 3 6; stroke-opacity: 0.6; }
.obs-sight { stroke: var(--color-primary); stroke-opacity: 0.5; stroke-width: 1; stroke-dasharray: 6 5; }
.obs-planet-halo { fill: none; stroke: var(--color-primary); stroke-opacity: 0.35; stroke-width: 1; stroke-dasharray: 2 4; }
.obs-planet { fill: var(--color-primary); }
.obs-planet-label { fill: var(--color-text-tertiary); font-size: 12px; text-anchor: middle; }
.obs-tick { fill: var(--color-text-disabled); font-size: 10px; font-family: var(--font-mono, Consolas, monospace); }
.obs-sat { fill: var(--color-text-tertiary); }
.obs-node { fill: none; stroke: var(--color-primary); stroke-width: 2; }
.obs-node-core { fill: var(--color-primary); }
.obs-callout { stroke: var(--color-text-tertiary); stroke-width: 1; stroke-dasharray: 3 4; }
.obs-callout-text { fill: var(--color-text-secondary); font-size: 12px; text-anchor: end; }

.obs-fig {
  font-family: var(--font-mono, 'JetBrains Mono', Consolas, monospace);
  font-size: 12px;
  color: var(--color-text-tertiary);
  letter-spacing: 1px;
  border-top: 1px solid var(--color-border-light);
  padding-top: 10px;
  margin-top: 8px;
}

/* 印章 */
.obs-stamp {
  position: absolute;
  right: 34px;
  bottom: 52px;
  transform: rotate(-8deg);
  border: 2px solid var(--color-error);
  border-radius: 4px;
  padding: 5px 12px;
  text-align: center;
  opacity: 0.55;
  pointer-events: none;
}
.obs-stamp span { display: block; }
.obs-stamp span:first-child {
  font-family: 'Noto Serif SC', 'Source Han Serif SC', 'Songti SC', 'SimSun', serif;
  font-size: 15px;
  font-weight: 700;
  color: var(--color-error);
  letter-spacing: 6px;
}
.obs-stamp-en {
  font-family: var(--font-mono, Consolas, monospace);
  font-size: 10px;
  letter-spacing: 3px;
  color: var(--color-error);
}

/* ---- 右侧动作条列 ---- */
.obs-side {
  flex: 1;
  min-width: 260px;
  max-width: 340px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding-top: 26px;
}
.obs-side-label {
  font-family: var(--font-mono, 'JetBrains Mono', Consolas, monospace);
  font-size: 12px;
  letter-spacing: 3px;
  color: var(--color-text-tertiary);
}
.obs-paste { display: flex; gap: 8px; }
.obs-rule { height: 1px; background: var(--color-border-light); margin: 4px 0; }

.obs-recent { display: flex; flex-direction: column; gap: 4px; }
.obs-recent-item {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 2px;
  padding: 8px 10px;
  background: transparent;
  border: 1px solid var(--color-border-light);
  border-radius: var(--radius-sm, 6px);
  cursor: pointer;
  text-align: left;
  transition: border-color var(--transition-fast), background-color var(--transition-fast);
}
.obs-recent-item:hover { border-color: var(--color-primary-border); background: var(--color-hover); }
.obs-recent-name { font-size: 14px; font-weight: 600; color: var(--color-text); }
.obs-recent-path {
  font-size: 12px;
  color: var(--color-text-tertiary);
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.obs-recent-empty {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 4px;
  color: var(--color-text-tertiary);
  font-size: 12px;
}
.obs-recent-glyph { width: 34px; height: 22px; flex-shrink: 0; }
.obs-foot-links {
  margin-top: auto;
  display: flex;
  gap: 14px;
}
.obs-link {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  background: transparent;
  border: none;
  padding: 4px 2px;
  font-size: 12px;
  color: var(--color-text-tertiary);
  cursor: pointer;
}
.obs-link:hover { color: var(--color-primary); }
</style>
