/**
 * 后端调用桥：`window.go.main.ByteGitApp.Method(args...)` → Tauri invoke。
 *
 * 前端保持既定调用约定（全局 ByteGitApp 对象 + PascalCase 方法名），
 * 本模块导出同名 Proxy，由 main.ts 安装到 `window.go.main.ByteGitApp`，
 * 方法调用转发为 `invoke(命令名, 参数)`：
 * - 方法名 PascalCase → Tauri 命令 snake_case（GitStatus → git_status）
 * - 位置参数按 SIGNATURES 表映射为命令形参（未登记的命令回退 argN 映射）
 * - Rust 侧尾部 repo: Option<String> 参数由后端当前仓库状态解析，无需前端传
 */

import { invoke } from '@tauri-apps/api/core'

/** 已实现命令的形参表（Go 绑定参数名 camelCase；Tauri 自动映射 snake_case 形参）。 */
const SIGNATURES: Record<string, string[]> = {
    // ---- 根命令（version/config）----
    Version: [],
    ConfigGet: ['key'],
    ConfigSet: ['key', 'value'],
    CancelTask: [],
    // ---- 根命令（Phase 1 遗留，待对应域落地后由域条目替换）----
    GitStatus: ['repo'],
    BranchClean: ['repo', 'dryRun'],
    IgnoreList: [],
    IgnoreAdd: ['repo', 'template'],
    // ---- commit 域（提交台 + 历史操作）----
    CommitStatus: ['repo'],
    CommitStage: ['repo', 'paths'],
    CommitUnstage: ['repo', 'paths'],
    CommitStageAll: ['repo'],
    CommitUnstageAll: ['repo'],
    CommitDiscardTracked: ['repo', 'paths'],
    CommitDiscardUntracked: ['repo', 'paths'],
    CommitStagedDiff: ['repo'],
    CommitGenerate: ['repo'],
    CommitDo: ['repo', 'message', 'noVerify'],
    CommitPush: ['force'],
    PushStatus: [],
    CommitCherryPick: ['hash'],
    CommitRevert: ['hash'],
    CommitResetPreview: ['hash'],
    CommitReset: ['hash', 'mode'],
    CommitCreateBranch: ['hash', 'branchName'],
    // ---- system 域（设置/规则/代码片段/下载）----
    SettingsGet: [],
    SettingsReload: [],
    SettingsSave: ['patch'],
    AiPromptGet: ['name'],
    AiPromptSave: ['name', 'content'],
    AiPromptReset: ['name'],
    RulesRead: [],
    RulesSave: ['content'],
    RulesReset: [],
    RulesPath: [],
    SnippetsList: [],
    SnippetsAdd: ['s'],
    SnippetsUpdate: ['s'],
    SnippetsDelete: ['id'],
    SnippetsToggleFavorite: ['id'],
    SnippetsFilePath: [],
    DownloadReleases: ['owner', 'repo'],
    // ---- ignore 域（.gitignore 管理）----
    IgnoreAddPreview: ['langs', 'source'],
    IgnoreAddApply: ['langs', 'source'],
    IgnoreWrite: ['content'],
    IgnoreListTemplates: [],
    IgnoreListGithubTemplates: [],
    IgnoreFileStatuses: [],
    IgnoreLoadCategories: [],
    IgnoreSaveCategories: ['cats'],
    IgnoreResetCategories: [],
    IgnoreGithubSync: [],
    IgnoreGithubReset: [],
    IgnoreSourcesStatus: [],
    IgnoreTemplateCopy: ['name'],
    IgnoreTemplateDelete: ['name'],
    IgnoreTemplateSave: ['name', 'content'],
    IgnoreTemplateView: ['name', 'source'],
    IgnoreCheck: ['filePath'],
    IgnoreCheckBatch: ['files'],
    IgnoreListFiles: [],
    IgnoreRefactorPreview: [],
    IgnoreRefactorApply: [],
    // ---- rewrite 域（AI 改写 + 元数据改写 + JSON 编辑器）----
    RewriteExport: ['repo', 'mode', 'n', 'skip'],
    RewriteEntryDiff: ['repo', 'hash'],
    RewriteRulesLoad: [],
    RewriteRulesSave: ['rules'],
    RewriteRulesReset: [],
    RewriteRuleGenerate: ['desc'],
    RewriteGenerate: ['repo', 'entry'],
    RewriteGenerateAll: ['repo', 'entries'],
    RewriteApplyPreview: ['repo', 'entries'],
    RewriteApply: ['repo', 'entries', 'autoBackup'],
    CommitCount: ['repo'],
    MetaRewriteExport: ['repo', 'opts'],
    MetaRewritePreview: ['repo', 'entries'],
    MetaRewriteApply: ['repo', 'entries', 'syncRefs', 'autoBackup'],
    MetaRewriteApplyRule: ['entries', 'rule'],
    MetaRewriteRemap: ['entries', 'dateRange', 'mode', 'dailyLimit', 'activeHour'],
    MetaRewriteDistributeByDays: ['entries', 'allocs', 'mode', 'activeHour', 'sizeWeight'],
    JsonEditorPath: [],
    JsonEditorRead: [],
    JsonEditorWrite: ['content'],
    JsonEditorRemove: [],
    // ---- secret 域（密钥扫描/历史抹除 + AI 审查 + OCR 配置）----
    SecretCheck: [],
    SecretScan: [],
    SecretFileContent: ['commit', 'filePath'],
    SecretPlatformHint: ['ruleId'],
    SecretRemoveFromWorkingTree: ['dto'],
    SecretScrubHistory: ['secret', 'autoBackup'],
    RewriteAutoBackupDefault: [],
    ReviewCheck: [],
    ReviewRun: ['dto'],
    OcrConfigGet: [],
    OcrConfigGetRaw: [],
    OcrConfigSave: ['patch'],
    OcrConfigSaveRaw: ['content'],
    // ---- license 域（开源许可证）----
    LicenseListTemplates: [],
    LicenseTemplateList: [],
    LicenseTemplateView: ['name'],
    LicenseTemplateSave: ['name', 'content'],
    LicenseTemplateDelete: ['name'],
    LicenseCreate: ['spdxId', 'author', 'year'],
    LicenseDetect: [],
    LicenseAuditDeps: [],
    LicenseRead: [],
    LicenseWrite: ['content'],
    // ---- gitops 域（配置/远端/分支/标签/钩子/stash）----
    GitConfigList: [],
    GitConfigSet: ['scope', 'key', 'value'],
    GitConfigUnset: ['scope', 'key'],
    RemoteList: [],
    RemoteSetURL: ['name', 'url'],
    RemoteAdd: ['name', 'url'],
    RemoteRemove: ['name'],
    BranchList: [],
    BranchCreate: ['name'],
    BranchSwitch: ['name'],
    BranchDelete: ['name'],
    BranchDeleteForce: ['name'],
    BranchSetUpstream: ['local', 'remote'],
    BranchRename: ['oldName', 'newName'],
    TagList: [],
    TagCreate: ['name', 'message', 'commit'],
    TagDelete: ['name'],
    TagPush: ['remote', 'names'],
    HookList: ['scope'],
    HookInstallScript: ['name', 'lang', 'scriptContent', 'scope'],
    HookRemove: ['name', 'scope'],
    HookReadScript: ['name', 'scope'],
    HookExec: ['name', 'scope'],
    HookConfigInfo: [],
    HookSetHooksPath: ['scope', 'value'],
    StashList: [],
    StashSave: ['name'],
    StashPop: ['index'],
    StashDrop: ['index'],
    StashDiff: ['index'],
    // ---- backup/archive/log 域 ----
    ArchiveRun: ['dto'],
    ArchiveList: [],
    ArchiveDelete: ['name'],
    FullBackupList: [],
    FullBackupCreate: [],
    FullBackupDelete: ['name'],
    FullBackupFileList: ['backupName', 'subPath'],
    FullBackupFileRead: ['backupName', 'filePath'],
    LogFetch: ['repo', 'limit', 'before'],
    LogFetchByDate: ['repo', 'date'],
    LogDiff: ['repo', 'hash'],
    CommitTotal: [],
    CommitStat: ['commit'],
    CommitFileDiff: ['commit', 'filePath'],
    CommitFileTree: ['commit'],
    CommitReadFile: ['commit', 'filePath'],
    // ---- info/stats 域 ----
    InfoSummary: ['repo'],
    InfoHealth: ['repo'],
    InfoLargeFiles: ['repo'],
    InfoSize: ['repo'],
    InfoBranches: ['repo'],
    InfoActivity: ['repo', 'days'],
    InfoAuthors: ['repo', 'days'],
    InfoChangelog: ['repo', 'from'],
    InfoHotspots: ['repo', 'days', 'limit'],
    InfoTodos: ['repo'],
    StatsGet: ['dir', 'since', 'until', 'author'],
    // ---- repo 域（仓库/文件浏览）----
    GetCurrentRepo: [],
    GetRepoName: ['repo'],
    CurrentRepoInfo: [],
    SwitchRepo: ['dir'],
    ListRecentRepos: [],
    RemoveRecentRepo: ['path'],
    SetRecentPinned: ['path', 'pinned'],
    SetRecentOrder: ['recent', 'pinned'],
    PickRepoFolder: [],
    RepoFileList: ['subPath'],
    RepoFileSearch: ['query'],
    RepoReadFile: ['relPath'],
    RepoReadme: [],
    RepoReadImage: ['relPath'],
    OpenInExplorer: ['path'],
    OpenInBrowser: ['url'],
    // ---- reference 域（reference.exe 集成）----
    ReferenceCheck: [],
    ReferenceInit: ['req'],
    ReferenceList: [],
    ReferenceMap: [],
    ReferenceRemove: ['name', 'purge'],
    ReferenceUpdate: ['name'],
    ReferenceAdd: ['req'],
    ReferenceDoctor: [],
    ReferenceSettings: [],
    ReferenceScc: ['name'],
    ReferenceCachePath: ['name', 'filePath'],
    ReferenceFileList: ['name', 'subPath'],
    ReferenceFileRead: ['name', 'filePath'],
    ReferenceFileSearch: ['name', 'query'],
    ReferenceWikiList: ['repoName'],
    ReferenceWikiFile: ['repoName', 'file'],
    ReferenceWikiSync: [],
    ReferenceWikiStatus: [],
    ReferenceWikiRemoteGet: [],
    ReferenceWikiRemoteSet: ['url'],
    ReferenceWikiRestore: ['path'],
    ReferenceWikiTrash: [],
    ReferenceWikiCommit: [],
    // ---- live/cheat 域（实时监视 + 备忘录）----
    LiveStartWatch: [],
    LiveStopWatch: [],
    LiveStat: [],
    LiveFileDiff: ['filePath'],
    LiveStagedFileDiff: ['filePath'],
    CheatEnsure: [],
    CheatPull: [],
    CheatList: [],
    CheatRead: ['relPath'],
    // ---- terminal 域（PTY 终端）----
    TerminalStart: ['shell', 'cwd', 'cols', 'rows'],
    TerminalInput: ['b64'],
    TerminalResize: ['cols', 'rows'],
    TerminalStop: [],
    TerminalDetectShell: [],
    // ---- github 域（GitHub REST 集成）----
    GhActionsList: [],
    GhActionsJobs: ['runId'],
    GhActionsJobLogs: ['jobId'],
    GhActionsRerun: ['runId'],
    GhActionsCancel: ['runId'],
    GhIssueList: ['state'],
    GhIssueCreate: ['title', 'body'],
    GhIssueClose: ['number'],
    GhIssueComment: ['number', 'body'],
    GhIssueLabels: [],
    GhIssueAddLabels: ['number', 'labels'],
    GhIssueRemoveLabel: ['number', 'label'],
    GhPRList: ['state'],
    GhPRCreate: ['title', 'head', 'base', 'body'],
    GhPRComments: ['number'],
    GhPRCreateComment: ['number', 'body'],
    GhPRFiles: ['number'],
    GhPRReview: ['number', 'event', 'body'],
    GhPRMerge: ['number', 'strategy'],
    GhPRCheckout: ['number'],
    GhReleaseList: [],
    GhReleaseCreate: ['tag', 'title', 'body', 'isDraft', 'isPre'],
    GhReleaseDelete: ['releaseId'],
    GhReleaseDeleteAsset: ['assetId'],
    GhReleaseUploadAsset: ['releaseId', 'filePath'],
    GhSearchRepos: ['query'],
    GhSearchCode: ['query'],
    GhTrendingProjects: ['timeframe', 'language', 'spokenLang'],
    GhTrendingLanguages: [],
    GhSpokenLanguages: [],
    // ---- archive 补全（导出/选目录）----
    ArchiveExportToDir: ['ref', 'targetDir'],
    ArchivePickDir: [],
    // ---- 导出面补全（GhStatus/GhPRDiff/NpmAvailable）----
    GhStatus: [],
    GhPRDiff: ['number'],
    NpmAvailable: [],
    // ---- 窗口控制（Go WindowModule；Maximize=切换，Close=退出应用）----
    WindowMinimize: [],
    WindowMaximize: [],
    WindowClose: [],
    WindowIsMaximized: [],
    // ---- 零豁免补齐 ----
    NpmManage: ['action'],
    ReferenceClearCache: [],
    ShowFileAt: ['commitRef', 'filePath'],
    // ---- wallpaper 域（壁纸管理）----
    WallpaperUpload: ['srcPath'],
    WallpaperList: [],
    WallpaperDelete: ['filename'],
    WallpaperDataDir: [],
    WallpaperDataURL: ['filename'],
    WallpaperThumbURL: ['filename'],
    WallpaperPickFile: [],
    Abs: ['filename'],
    GhIssueComments: ['number'],
    TeardownPtyLocked: [],
}

/** PascalCase/camelCase → snake_case。
 *  两条边界规则：小写/数字→大写 插下划线；缩写尾边界（大写 + 大写开头小写结尾，如 GhPRList 的 PR|List）插下划线。
 *  例：GhPRList → gh_pr_list、RemoteSetURL → remote_set_url、OcrConfigGetRaw → ocr_config_get_raw。 */
function toCommandName(method: string): string {
    return method
        .replace(/^([A-Z])/, (c) => c.toLowerCase())
        .replace(/([a-z0-9])([A-Z])/g, '$1_$2')
        .replace(/([A-Z])([A-Z][a-z])/g, '$1_$2')
        .replace(/[A-Z]/g, (c) => c.toLowerCase())
}

/** 位置参数 → invoke 参数对象。 */
function marshal(method: string, args: unknown[]): Record<string, unknown> {
    const names = SIGNATURES[method]
    if (names) {
        const out: Record<string, unknown> = {}
        names.forEach((n, i) => {
            if (i < args.length) out[n] = args[i]
        })
        return out
    }
    // 未登记命令：单对象参数直接透传，否则按 arg0..N
    if (args.length === 1 && typeof args[0] === 'object' && args[0] !== null) {
        return args[0] as Record<string, unknown>
    }
    const out: Record<string, unknown> = {}
    args.forEach((v, i) => (out[`arg${i}`] = v))
    return out
}

function makeBridge(): unknown {
    return new Proxy(
        {},
        {
            get(_target, prop: string | symbol) {
                if (typeof prop !== 'string') return undefined
                return (...args: unknown[]) => invoke(toCommandName(prop), marshal(prop, args))
            },
        },
    )
}

export const ByteGitApp: unknown = makeBridge()

/** 安装到 window.go.main.ByteGitApp（幂等，不覆盖 Wails 原生注入）。 */
export function installBridge(): void {
    const w = window as unknown as { go?: Record<string, Record<string, unknown>> }
    if (w.go?.main?.ByteGitApp) return // 已有原生绑定
    w.go ??= {}
    w.go.main ??= {}
    w.go.main.ByteGitApp = ByteGitApp
}
