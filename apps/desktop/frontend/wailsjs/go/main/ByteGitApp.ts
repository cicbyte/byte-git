/**
 * Wails Go 绑定 → Tauri invoke 适配层。
 *
 * git-mate 前端通过 `window.go.main.ByteGitApp.Method(args...)` 调用后端；
 * byte-git 桌面端改用 Tauri。本模块导出同名 Proxy 对象，由 main.ts 安装到
 * `window.go.main.ByteGitApp`，方法调用转发为 `invoke(命令名, 参数)`：
 * - 方法名 camelCase → Tauri 命令 snake_case（GitStatus → git_status）
 * - 位置参数按 SIGNATURES 表映射为命令形参（未登记的命令回退 argN 映射）
 */

import { invoke } from '@tauri-apps/api/core'

/** 已实现命令的形参表（camelCase 键，Tauri 自动映射 snake_case 形参）。 */
const SIGNATURES: Record<string, string[]> = {
    Version: [],
    InfoSummary: ['repo'],
    GitStatus: ['repo'],
    BranchList: ['repo'],
    BranchClean: ['repo', 'dryRun'],
    StashList: ['repo', 'search'],
    StashSave: ['repo', 'name'],
    StashPop: ['repo', 'name'],
    LicenseList: [],
    LicenseCreate: ['repo', 'name', 'author', 'year'],
    IgnoreList: [],
    IgnoreAdd: ['repo', 'template'],
    ConfigGet: ['key'],
    ConfigSet: ['key', 'value'],
}

/** camelCase → snake_case（GitStatus → git_status）。 */
function toCommandName(method: string): string {
    return method.replace(/[A-Z]/g, (c) => '_' + c.toLowerCase())
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
