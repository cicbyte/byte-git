<script setup>
import { ref, computed, onMounted, watch } from 'vue'
import { message as AMessage, Modal } from 'ant-design-vue'
import {
  ThunderboltOutlined, DeleteOutlined, SaveOutlined, PlayCircleOutlined,
} from '@ant-design/icons-vue'
import CodeViewer from '../common/CodeViewer.vue'

/**
 * Hook 管理 — 列表 + CodeMirror 编辑器 + 安装/卸载/测试运行。
 */
const app = window.go?.main?.ByteGitApp

const hooks = ref([])
const loading = ref(false)
const activeHook = ref('')
const activeScope = ref('local')     // "local" | "global"
const script = ref('')
const scriptLang = ref('shell')
const scriptLoading = ref(false)
const saving = ref(false)
const testing = ref(false)
const dirty = ref(false)
const configInfo = ref(null)

// whether the current scope is configured (global needs hooksPath; local always works)
const scopeConfigured = computed(() => {
  if (!configInfo.value) return true
  if (activeScope.value === 'global') return configInfo.value.hasGlobalHooksPath
  return true // local always has .git/hooks fallback
})

/**
 * Per-hook per-language sample templates.
 * Structure: { hookName: { shell: "...", python: "...", node: "..." } }
 * Falls back to a generic template per language when a specific hook isn't listed.
 */
const hookTemplates = {
  'pre-commit': {
    shell: '#!/bin/sh\n# pre-commit: 提交前检查（exit 非 0 中止提交）\n# 常见用途: lint、禁止调试代码提交\n\necho "[pre-commit] 检查通过"\nexit 0\n',
    python: '#!/usr/bin/env python3\n# pre-commit: 提交前检查\nimport subprocess, sys\n\n# 示例: 检查是否有 debugger 残留\nresult = subprocess.run(\n    ["git", "diff", "--cached", "--name-only"],\n    capture_output=True, text=True\n)\nfor f in result.stdout.strip().split("\\n"):\n    if not f:\n        continue\n    with open(f, errors="ignore") as fh:\n        if "pdb.set_trace()" in fh.read():\n            print(f"[pre-commit] {f} 含有 debugger，请移除")\n            sys.exit(1)\n\nprint("[pre-commit] 检查通过")\nsys.exit(0)\n',
    node: '#!/usr/bin/env node\n// pre-commit: 提交前检查\nconst { execSync } = require("child_process");\n\n// 示例: 检查是否有 console.log 残留\nconst files = execSync("git diff --cached --name-only", { encoding: "utf-8" })\n  .trim().split("\\n").filter(Boolean);\n\nconst fs = require("fs");\nfor (const f of files) {\n  const content = fs.readFileSync(f, "utf-8").catch?.("") || "";\n  if (content.includes("console.log(") && !f.endsWith(".min.js")) {\n    console.error(`[pre-commit] ${f} 含有 console.log，请移除`);\n    process.exit(1);\n  }\n}\n\nconsole.log("[pre-commit] 检查通过");\nprocess.exit(0);\n',
  },
  'commit-msg': {
    shell: '#!/bin/sh\n# commit-msg: 校验 commit message 格式\n# 常见用途: 强制 conventional commits 规范\n\nmsg_file="$1"\nfirst_line=$(head -1 "$msg_file")\n\n# 校验: type(scope): subject\npattern="^(feat|fix|docs|style|refactor|perf|test|chore|build|ci|revert)(\\([a-z-]+\\))?: .+"\n\nif ! echo "$first_line" | grep -qE "$pattern"; then\n  echo "[commit-msg] 提交信息不符合规范！"\n  echo "  期望: type(scope): subject"\n  echo "  示例: feat(auth): 添加 OAuth 登录"\n  echo "  实际: $first_line"\n  exit 1\nfi\n\nexit 0\n',
    python: '#!/usr/bin/env python3\n# commit-msg: 校验 commit message 格式\nimport re, sys\n\nmsg_file = sys.argv[1]\nwith open(msg_file) as f:\n    first_line = f.readline().strip()\n\n# conventional commits: type(scope): subject\npattern = r"^(feat|fix|docs|style|refactor|perf|test|chore|build|ci|revert)(\\([a-z-]+\\))?: .+"\n\nif not re.match(pattern, first_line):\n    print("[commit-msg] 提交信息不符合规范！")\n    print(f"  期望: type(scope): subject")\n    print(f"  示例: feat(auth): 添加 OAuth 登录")\n    print(f"  实际: {first_line}")\n    sys.exit(1)\n\nsys.exit(0)\n',
    node: '#!/usr/bin/env node\n// commit-msg: 校验 commit message 格式\nconst fs = require("fs");\n\nconst msgFile = process.argv[2];\nconst firstLine = fs.readFileSync(msgFile, "utf-8").split("\\n")[0];\n\n// conventional commits: type(scope): subject\nconst pattern = /^(feat|fix|docs|style|refactor|perf|test|chore|build|ci|revert)(\\([a-z-]+\\))?: .+/;\n\nif (!pattern.test(firstLine)) {\n  console.error("[commit-msg] 提交信息不符合规范！");\n  console.error("  期望: type(scope): subject");\n  console.error("  示例: feat(auth): 添加 OAuth 登录");\n  console.error(`  实际: ${firstLine}`);\n  process.exit(1);\n}\n\nprocess.exit(0);\n',
  },
  'pre-push': {
    shell: '#!/bin/sh\n# pre-push: 推送前检查（exit 非 0 中止推送）\n# 常见用途: 阻止向 main/master 直接推送\n# stdin 格式: <local ref> <local sha> <remote ref> <remote sha>\n\nwhile read local_ref local_sha remote_ref remote_sha; do\n  if echo "$remote_ref" | grep -qE "refs/heads/(main|master)$"; then\n    echo "[pre-push] 禁止直接推送到 $remote_ref，请使用 PR"\n    exit 1\n  fi\ndone\n\necho "[pre-push] 推送检查通过"\nexit 0\n',
    python: '#!/usr/bin/env python3\n# pre-push: 推送前检查\nimport sys\n\nfor line in sys.stdin:\n    parts = line.strip().split()\n    if len(parts) < 4:\n        continue\n    remote_ref = parts[2]\n    if remote_ref.endswith(("refs/heads/main", "refs/heads/master")):\n        print(f"[pre-push] 禁止直接推送到 {remote_ref}，请使用 PR")\n        sys.exit(1)\n\nprint("[pre-push] 推送检查通过")\nsys.exit(0)\n',
    node: '#!/usr/bin/env node\n// pre-push: 推送前检查\nconst readline = require("readline");\nconst rl = readline.createInterface({ input: process.stdin });\n\nrl.on("line", (line) => {\n  const parts = line.trim().split(/\\s+/);\n  if (parts.length < 4) return;\n  const remoteRef = parts[2];\n  if (remoteRef.endsWith("refs/heads/main") || remoteRef.endsWith("refs/heads/master")) {\n    console.error(`[pre-push] 禁止直接推送到 ${remoteRef}，请使用 PR`);\n    process.exit(1);\n  }\n});\n\nrl.on("close", () => {\n  console.log("[pre-push] 推送检查通过");\n  process.exit(0);\n});\n',
  },
  'post-commit': {
    shell: '#!/bin/sh\n# post-commit: 提交完成后触发\n# 常见用途: 通知、统计\necho "[post-commit] 提交完成"\nexit 0\n',
    python: '#!/usr/bin/env python3\n# post-commit: 提交完成后触发\nimport subprocess\n\nsha = subprocess.run(["git", "rev-parse", "--short", "HEAD"], capture_output=True, text=True).stdout.strip()\nprint(f"[post-commit] 提交完成 {sha}")\n',
    node: '#!/usr/bin/env node\n// post-commit: 提交完成后触发\nconst { execSync } = require("child_process");\nconst sha = execSync("git rev-parse --short HEAD", { encoding: "utf-8" }).trim();\nconsole.log(`[post-commit] 提交完成 ${sha}`);\n',
  },
}

// generic templates per language (for hooks not in hookTemplates)
const genericTemplates = {
  shell: function(name) {
    return '#!/bin/sh\n# ' + name + ' hook\n# exit 0 = 放行，非 0 = 中止 git 操作\necho "[' + name + '] hook triggered"\nexit 0\n'
  },
  python: function(name) {
    return '#!/usr/bin/env python3\n# ' + name + ' hook\nimport sys\nprint("[' + name + '] hook triggered")\nsys.exit(0)\n'
  },
  node: function(name) {
    return '#!/usr/bin/env node\n// ' + name + ' hook\nconsole.log("[' + name + '] hook triggered");\nprocess.exit(0);\n'
  },
}

function getTemplate(hookName, lang) {
  if (!lang) lang = 'shell'
  // specific hook + language template
  if (hookTemplates[hookName] && hookTemplates[hookName][lang]) {
    return hookTemplates[hookName][lang]
  }
  // generic per-language template
  return genericTemplates[lang](hookName || 'git')
}

async function loadConfigInfo() {
  if (!app?.HookConfigInfo) return
  try { configInfo.value = await app.HookConfigInfo() } catch { configInfo.value = null }
}


const activeHookInfo = computed(() =>
  hooks.value.find((h) => h.name === activeHook.value) || null,
)

// dynamic filename for CodeViewer syntax highlighting (by language)
const editorFilename = computed(() => {
  const lang = scriptLang.value
  if (lang === 'python') return 'hook.py'
  if (lang === 'node') return 'hook.js'
  return 'hook.sh'
})

// human-readable type labels for badges
function typeLabel(t) {
  const m = { lua: 'Lua', shell: 'Shell', python: 'Python', node: 'Node', script: 'Script', sample: 'Sample', '': '未安装' }
  return m[t] || t
}

async function loadHooks() {
  if (!app?.HookList) return
  loading.value = true
  try {
    hooks.value = await app.HookList(activeScope.value)
    activeHook.value = hooks.value.length ? hooks.value[0].name : ''
    if (activeHook.value) await loadScript()
    else { script.value = ''; scriptLang.value = 'shell' }
  } catch { hooks.value = [] }
  finally { loading.value = false }
}

async function loadScript() {
  if (!activeHook.value || !app?.HookReadScript) return
  scriptLoading.value = true
  const t = activeHookInfo.value?.type
  if (t === 'python') scriptLang.value = 'python'
  else if (t === 'node') scriptLang.value = 'node'
  else scriptLang.value = 'shell'
  try {
    const content = await app.HookReadScript(activeHook.value, activeScope.value)
    script.value = content || getTemplate(activeHook.value, scriptLang.value)
    dirty.value = false
  } catch { script.value = getTemplate(activeHook.value, scriptLang.value) }
  finally { scriptLoading.value = false }
}

function selectHook(name) {
  if (dirty.value && !confirm('当前脚本未保存，切换将丢弃改动，确定？')) return
  activeHook.value = name
  loadScript()
}

function onScriptChange(v) {
  script.value = v
  dirty.value = true
}

async function save() {
  if (!activeHook.value) return
  if (!scopeConfigured.value) {
    AMessage.warning('请先在「Git 配置 → Hook」页设置全局 core.hooksPath')
    return
  }
  saving.value = true
  try {
    const lang = scriptLang.value
    await app.HookInstallScript(activeHook.value, lang, script.value, activeScope.value)
    AMessage.success(`已安装 hook: ${activeHook.value}`)
    dirty.value = false
    const idx = hooks.value.findIndex((h) => h.name === activeHook.value)
    if (idx >= 0) {
      hooks.value[idx] = { ...hooks.value[idx], enabled: true, type: lang }
    }
  } catch (e) {
    AMessage.error('安装失败: ' + String(e?.message || e))
  } finally {
    saving.value = false
  }
}

function confirmRemove() {
  if (!activeHook.value || !activeHookInfo.value?.enabled) return
  Modal.confirm({
    title: `卸载 hook「${activeHook.value}」？`,
    content: '将删除 .git/hooks/ 下的脚本文件和配置记录。',
    okText: '卸载', okType: 'danger', cancelText: '取消',
    async onOk() {
      try {
        await app.HookRemove(activeHook.value, activeScope.value)
        AMessage.success('已卸载')
        script.value = getTemplate(activeHook.value, scriptLang.value)
        dirty.value = false
        // Local update instead of loadHooks().
        const idx = hooks.value.findIndex((h) => h.name === activeHook.value)
        if (idx >= 0) {
          hooks.value[idx] = { ...hooks.value[idx], enabled: false, type: 'sample', source: '' }
        }
      } catch (e) { AMessage.error('卸载失败: ' + String(e?.message || e)) }
    },
  })
}

async function testRun() {
  if (!activeHook.value || !app?.HookExec) return
  testing.value = true
  try {
    const result = await app.HookExec(activeHook.value, activeScope.value)
    if (result.exitCode === 0) {
      AMessage.success(result.output || '执行成功')
    } else {
      AMessage.error(result.error || result.output || `执行失败 (exit ${result.exitCode})`)
    }
  } catch (e) {
    AMessage.error('执行失败: ' + String(e?.message || e))
  } finally {
    testing.value = false
  }
}

const statusBadge = {
  lua: { text: 'Lua', color: 'blue' },
  script: { text: 'Script', color: 'green' },
  sample: { text: 'Sample', color: 'default' },
  '': { text: '未安装', color: 'default' },
}

// when user switches language selector, reload template if content is empty or unchanged
watch(scriptLang, (newLang, oldLang) => {
  if (!activeHook.value) return
  if (!dirty.value) {
    script.value = getTemplate(activeHook.value, newLang)
  }
})

onMounted(async () => {
  await loadConfigInfo()
  await loadHooks()
})
watch(activeScope, () => { loadHooks() })
</script>

<template>
  <div class="hk-form">
    <!-- scope tabs -->
    <div class="hk-scope-tabs">
      <a-radio-group v-model:value="activeScope" button-style="solid" size="small">
        <a-radio-button value="local">仓库级</a-radio-button>
        <a-radio-button value="global">全局级</a-radio-button>
      </a-radio-group>
      <span class="hk-scope-hint">
        {{ activeScope === 'local'
          ? '仓库 hooks 目录（默认 .git/hooks），优先级高于全局'
          : '全局 hooks 目录（需配置 core.hooksPath），对所有仓库生效' }}
      </span>
    </div>

    <div v-if="loading" class="hk-center"><a-spin /></div>

    <!-- global scope not configured -->
    <div v-else-if="!scopeConfigured" class="hk-center hk-not-configured">
      <div class="hk-nc-title">全局 core.hooksPath 未设置</div>
      <div class="hk-nc-desc">请先在「Git 配置 → Hook」页设置全局 core.hooksPath，再管理全局 hook。</div>
    </div>

    <div v-else class="hk-layout">
        <!-- left: hook list -->
        <aside class="hk-list">
          <div class="hk-list-head">Hook 槽位（{{ hooks.length }}）</div>
          <div class="hk-list-body">
            <div
              v-for="h in hooks" :key="h.name"
              class="hk-item" :class="{ active: h.name === activeHook }"
              @click="selectHook(h.name)"
            >
              <span class="hk-name">{{ h.name }}</span>
              <span class="hk-badge" :class="'hb-' + (h.type || 'none')">
                {{ typeLabel(h.type) }}
              </span>
            </div>
          </div>
        </aside>

        <!-- right: editor -->
        <section class="hk-editor-area">
          <div class="hk-editor-head">
            <span class="hk-active-name">{{ activeHook || '—' }}</span>
            <a-select v-model:value="scriptLang" size="small" style="width: 100px">
              <a-select-option value="shell">Shell</a-select-option>
              <a-select-option value="python">Python</a-select-option>
              <a-select-option value="node">Node</a-select-option>
            </a-select>
            <span v-if="dirty" class="hk-dirty">未保存 ●</span>
            <div class="hk-actions">
              <a-button size="small" type="primary" :loading="saving" :disabled="!activeHook" @click="save">
                <template #icon><SaveOutlined /></template>保存并安装
              </a-button>
              <a-button size="small" :loading="testing" :disabled="!activeHook || !activeHookInfo?.enabled" @click="testRun">
                <template #icon><PlayCircleOutlined /></template>测试
              </a-button>
              <a-button size="small" danger :disabled="!activeHookInfo?.enabled" @click="confirmRemove">
                <template #icon><DeleteOutlined /></template>卸载
              </a-button>
            </div>
          </div>
          <div class="hk-editor">
            <a-spin v-if="scriptLoading" class="hk-spin" />
            <CodeViewer
              v-else
              :code="script"
              :filename="editorFilename"
              editable
              @update:modelValue="onScriptChange"
            />
          </div>
          <!-- hook config info panel (read-only; edit in Git 配置 page) -->
          <div v-if="configInfo" class="hk-config-panel">
            <div class="hk-config-row">
              <span class="hk-config-label">当前 Hooks 目录</span>
              <code class="hk-config-val">{{ configInfo.hooksPathEffective || configInfo.defaultHooksDir }}</code>
              <span v-if="!configInfo.hooksPathLocal && !configInfo.hooksPathGlobal" class="hk-config-hint">
                在「Git 配置 → Hook」页设置 core.hooksPath
              </span>
            </div>
            <div v-if="configInfo.hooksPathLocal" class="hk-config-row">
              <span class="hk-config-scope local">仓库级</span>
              <code class="hk-config-val">{{ configInfo.hooksPathLocal }}</code>
            </div>
            <div v-if="configInfo.hooksPathGlobal" class="hk-config-row">
              <span class="hk-config-scope global">全局级</span>
              <code class="hk-config-val">{{ configInfo.hooksPathGlobal }}</code>
            </div>
          </div>

          <div class="hk-hint">
            <ThunderboltOutlined /> 脚本首行需声明 shebang（如 <code>#!/bin/sh</code>），退出码 0 = 放行 / 非 0 = 中止。
          </div>
        </section>
      </div>

  </div>
</template>

<style scoped>
.hk-form { flex: 1; display: flex; flex-direction: column; padding: 20px 24px; background: var(--color-background); overflow: hidden; min-height: 0; }
.hk-scope-tabs { display: flex; align-items: center; gap: var(--spacing-sm); margin-bottom: var(--spacing-md); }
.hk-scope-hint { font-size: 11px; color: var(--color-text-tertiary); }
.hk-not-configured { gap: 8px; }
.hk-nc-title { font-size: 14px; font-weight: 600; color: var(--color-text); }
.hk-nc-desc { font-size: 12px; color: var(--color-text-tertiary); }
.hk-center { flex: 1; display: flex; align-items: center; justify-content: center; }
.hk-layout { flex: 1; display: flex; gap: var(--spacing-md); min-height: 0; }

/* left: hook list */
.hk-list { width: 200px; flex-shrink: 0; background: var(--color-surface); border: 1px solid var(--color-border); border-radius: var(--radius-md); display: flex; flex-direction: column; overflow: hidden; }
.hk-list-head { display: flex; align-items: center; justify-content: space-between; padding: 6px 8px 6px 12px; font-size: 12px; font-weight: 600; color: var(--color-text-secondary); border-bottom: 1px solid var(--color-border); flex-shrink: 0; }
.hk-list-body { flex: 1; overflow-y: auto; padding: 4px; }
.hk-item {
  display: flex; align-items: center; justify-content: space-between;
  padding: 6px 10px; border-radius: var(--radius-xs); cursor: pointer;
  font-size: 12px; color: var(--color-text-secondary);
  transition: all var(--transition-fast);
}
.hk-item:hover { background: var(--color-hover); }
.hk-item.active { background: var(--color-primary-bg); color: var(--color-primary); font-weight: 500; }
.hk-badge { font-size: 9px; padding: 1px 5px; border-radius: 3px; font-weight: 600; flex-shrink: 0; }
.hb-lua { background: var(--color-info-bg); color: var(--color-info); }
.hb-shell { background: var(--color-success-bg); color: var(--color-success); }
.hb-python { background: var(--color-warning-bg); color: var(--color-warning); }
.hb-node { background: var(--color-success-bg); color: var(--color-success); }
.hb-script { background: var(--color-success-bg); color: var(--color-success); }
.hb-sample, .hb-none { background: var(--color-surface-raised); color: var(--color-text-tertiary); }
.hk-badges { display: flex; align-items: center; gap: 3px; flex-shrink: 0; }
.hk-scope { font-size: 8px; padding: 1px 4px; border-radius: 3px; font-weight: 600; }
.hs-local { background: var(--color-primary-bg); color: var(--color-primary); }
.hs-global { background: var(--color-accent-bg); color: var(--color-accent); }

/* right: editor */
.hk-editor-area { flex: 1; min-width: 0; display: flex; flex-direction: column; background: var(--color-surface); border: 1px solid var(--color-border); border-radius: var(--radius-md); overflow: hidden; }
.hk-editor-head {
  display: flex; align-items: center; gap: 8px; padding: 8px 12px;
  border-bottom: 1px solid var(--color-border); flex-shrink: 0;
}
.hk-active-name { font-family: 'Cascadia Code', monospace; font-size: 13px; color: var(--color-text); font-weight: 500; }
.hk-dirty { font-size: 11px; color: var(--color-warning); font-weight: 600; }
.hk-actions { margin-left: auto; display: flex; gap: 6px; }
.hk-editor { flex: 1; min-height: 0; position: relative; overflow: hidden; }
.hk-spin { position: absolute; inset: 0; display: flex; align-items: center; justify-content: center; }
.hk-hint {
  padding: 8px 12px; font-size: 11px; color: var(--color-text-tertiary);
  border-top: 1px solid var(--color-border-light); flex-shrink: 0;
  display: flex; align-items: center; gap: 4px;
}
.hk-hint code { font-family: 'Cascadia Code', monospace; color: var(--color-primary); background: var(--color-primary-bg); padding: 0 4px; border-radius: 3px; }

/* add hook modal */
.hk-add-body { display: flex; flex-direction: column; gap: 12px; }
.hk-add-hint { font-size: 12px; color: var(--color-text-secondary); line-height: 1.6; margin: 0; }
.hk-add-hint code { font-family: 'Cascadia Code', monospace; color: var(--color-primary); background: var(--color-primary-bg); padding: 0 4px; border-radius: 3px; }
.hk-add-actions { display: flex; justify-content: flex-end; gap: 8px; }
.hk-add-lang { display: flex; align-items: center; gap: 8px; }
.hk-add-lang-label { font-size: 12px; color: var(--color-text-secondary); flex-shrink: 0; }

/* config info panel */
.hk-config-panel {
  padding: 8px 12px; border-top: 1px solid var(--color-border-light);
  display: flex; flex-direction: column; gap: 4px; flex-shrink: 0;
  background: var(--color-background);
}
.hk-config-row { display: flex; align-items: center; gap: 6px; font-size: 11px; }
.hk-config-label { color: var(--color-text-tertiary); flex-shrink: 0; min-width: 90px; }
.hk-config-val { font-family: 'Cascadia Code', monospace; color: var(--color-text-secondary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; flex: 1; }
.hk-config-scope { font-size: 9px; padding: 1px 5px; border-radius: 3px; font-weight: 600; flex-shrink: 0; min-width: 40px; text-align: center; }
.hk-config-scope.local { background: var(--color-primary-bg); color: var(--color-primary); }
.hk-config-scope.global { background: var(--color-accent-bg); color: var(--color-accent); }
.hk-config-hint { font-size: 10px; color: var(--color-text-tertiary); margin-left: auto; }
</style>
