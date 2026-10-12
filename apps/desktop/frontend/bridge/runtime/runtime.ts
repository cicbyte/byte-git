/**
 * 事件桥：前端事件订阅 API → Tauri v2 事件。
 *
 * 前端沿用 `EventsOn/EventsOff/EventsEmit` 调用约定，
 * 本模块内部转发到 @tauri-apps/api/event，调用方无感。
 */

import { emit, listen, type UnlistenFn } from '@tauri-apps/api/event'

const listeners: Record<string, UnlistenFn[]> = {}

/**
 * 订阅事件（对应 Wails EventsOn，回调签名与 Wails 生成契约一致采用 any 透传）。
 * @param eventName 事件名
 * @param callback 负载回调（多参负载按可变参数展开）
 */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function EventsOn(eventName: string, callback: (...data: any[]) => void): void {
    listen(eventName, (event) => {
        const payload = (event as { payload?: unknown }).payload
        if (Array.isArray(payload)) {
            callback(...payload)
        } else {
            callback(payload)
        }
    }).then((un) => {
        (listeners[eventName] ??= []).push(un)
    })
}

/** 取消订阅（对应 Wails EventsOff，按事件名移除全部监听）。 */
export function EventsOff(eventName: string, ...additionalEventNames: string[]): void {
    const off = (name: string) => {
        ;(listeners[name] ?? []).forEach((un) => un())
        delete listeners[name]
    }
    off(eventName)
    additionalEventNames.forEach(off)
}

/** 发出事件（对应 Wails EventsEmit，广播给所有窗口与后端）。 */
export function EventsEmit(eventName: string, ...data: unknown[]): void {
    void emit(eventName, data.length === 1 ? data[0] : data)
}
