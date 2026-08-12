import { ClipboardSyncConfig } from '../types/network'

// EasyShare 剪贴板同步配置的旁路持久化。
//
// 由于 EasyTier 核心的 NetworkConfig 在后端存取回合（TOML 序列化）中会丢弃未知字段，
// clipboard_sync 不能塞进核心 NetworkConfig。这里用独立的 localStorage 命名空间，
// 按网络实例 instance_id 单独存储，从而完全不修改 EasyTier 核心代码。

const STORAGE_KEY = 'easyshareClipboardSyncConfigs'

export const DEFAULT_CLIPBOARD_SYNC: ClipboardSyncConfig = {
  enabled: false,
  file_transfer: false,
  device_name: '',
  network_name: '',
  sync_images: false,
  port: 12000,
}

type ClipboardSyncMap = Record<string, ClipboardSyncConfig>

function readAll(): ClipboardSyncMap {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return {}
    const parsed = JSON.parse(raw)
    return (parsed && typeof parsed === 'object' ? parsed : {}) as ClipboardSyncMap
  } catch {
    return {}
  }
}

function writeAll(map: ClipboardSyncMap): void {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(map))
}

export function getClipboardSync(instanceId: string): ClipboardSyncConfig {
  const stored = readAll()[instanceId]
  return stored ? { ...DEFAULT_CLIPBOARD_SYNC, ...stored } : { ...DEFAULT_CLIPBOARD_SYNC }
}

export function saveClipboardSync(instanceId: string, config: ClipboardSyncConfig): void {
  const map = readAll()
  map[instanceId] = config
  writeAll(map)
}

export function removeClipboardSync(instanceId: string): void {
  const map = readAll()
  if (instanceId in map) {
    delete map[instanceId]
    writeAll(map)
  }
}

// ---- Android 无障碍权限 ----
//
// Android 10 起后台应用读不到剪贴板，只有无障碍服务例外。所以在手机上必须
// 先引导用户授权，剪贴板同步才真正可用。
//
// Config.vue 同时被 Web 端和 Tauri 端复用，Web 端没有 __TAURI_INTERNALS__，
// 因此这里全部做运行时探测，不静态 import @tauri-apps/api，避免污染 Web 构建。

function tauriInvoke(): ((cmd: string, args?: any) => Promise<any>) | null {
  const internals = (globalThis as any).__TAURI_INTERNALS__
  if (internals && typeof internals.invoke === 'function') {
    return (cmd: string, args?: any) => internals.invoke(cmd, args)
  }
  return null
}

/** 当前是否运行在 Android 上的 Tauri 宿主中（决定是否显示授权入口）。 */
export function isAndroidHost(): boolean {
  if (!tauriInvoke()) return false
  const ua = (globalThis as any).navigator?.userAgent ?? ''
  return /android/i.test(ua)
}

/** 查询无障碍服务是否已开启；非 Android 环境恒为 true（无需该权限）。 */
export async function isAccessibilityEnabled(): Promise<boolean> {
  const invoke = tauriInvoke()
  if (!invoke) return true
  try {
    return await invoke('is_accessibility_enabled')
  } catch {
    return true
  }
}

/** 打开系统无障碍设置页，引导用户为本应用开启剪贴板监听服务。 */
export async function openAccessibilitySettings(): Promise<void> {
  const invoke = tauriInvoke()
  if (!invoke) return
  try {
    await invoke('open_accessibility_settings')
  } catch (e) {
    console.error('open accessibility settings failed', e)
  }
}
