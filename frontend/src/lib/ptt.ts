import { voiceManager, type VoiceInputMode } from './voice';

let currentKey: string | null = null;
let browserListenersActive = false;
let browserPTTKeyCode = 'Space';
let browserPTTModifiers = { ctrl: false, shift: false, alt: false, meta: false };
let evdevListenerActive = false;
let evdevUnlisten: (() => void) | null = null;
let globalShortcutRegistered: string | null = null;

const STORAGE_KEY = 'voice-settings';

export interface VoiceSettings {
  inputMode: VoiceInputMode;
  pttKey: string;
}

function getDefaultSettings(): VoiceSettings {
  return { inputMode: 'voice-activity', pttKey: 'Space' };
}

export function loadVoiceSettings(): VoiceSettings {
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    if (saved) {
      return { ...getDefaultSettings(), ...JSON.parse(saved) };
    }
  } catch {
    // ignore parse errors
  }
  return getDefaultSettings();
}

export function saveVoiceSettings(settings: VoiceSettings): void {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
}

// Check if we're running inside Tauri (IPC bridge exists)
function isTauriRuntime(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

// Map browser KeyboardEvent.code to key name for display and storage
function tauriKeyFromCode(code: string): string {
  if (code.startsWith('Key')) return code.slice(3);
  if (code.startsWith('Digit')) return code.slice(5);
  const map: Record<string, string> = {
    Space: 'Space',
    Backquote: 'Backquote',
    CapsLock: 'CapsLock',
    Tab: 'Tab',
    Backslash: 'Backslash',
    BracketLeft: 'BracketLeft',
    BracketRight: 'BracketRight',
    Semicolon: 'Semicolon',
    Quote: 'Quote',
    Comma: 'Comma',
    Period: 'Period',
    Slash: 'Slash',
    Minus: 'Minus',
    Equal: 'Equal',
  };
  if (map[code]) return map[code];
  if (code.startsWith('F') && /^F\d+$/.test(code)) return code;
  return code;
}

// Convert key name back to browser KeyboardEvent.code for fallback
function codeFromTauriKey(key: string): string {
  if (key.length === 1 && /[A-Z]/.test(key)) return `Key${key}`;
  if (key.length === 1 && /[0-9]/.test(key)) return `Digit${key}`;
  const map: Record<string, string> = {
    Space: 'Space',
    Backquote: 'Backquote',
    CapsLock: 'CapsLock',
    Tab: 'Tab',
    Backslash: 'Backslash',
    BracketLeft: 'BracketLeft',
    BracketRight: 'BracketRight',
    Semicolon: 'Semicolon',
    Quote: 'Quote',
    Comma: 'Comma',
    Period: 'Period',
    Slash: 'Slash',
    Minus: 'Minus',
    Equal: 'Equal',
  };
  if (map[key]) return map[key];
  return key;
}

// --- evdev-based PTT (Tauri on Linux, works on Wayland) ---

async function startEvdevPTT(key: string): Promise<boolean> {
  if (!isTauriRuntime()) return false;

  try {
    const { invoke } = await import('@tauri-apps/api/core');
    const { listen } = await import('@tauri-apps/api/event');

    await invoke('start_ptt', { key });

    // Listen for ptt-state events from the Rust evdev listener
    if (evdevUnlisten) {
      evdevUnlisten();
    }
    evdevUnlisten = await listen<boolean>('ptt-state', (event) => {
      voiceManager.setPTTActive(event.payload);
    });

    evdevListenerActive = true;
    return true;
  } catch (e) {
    console.warn('evdev PTT not available:', e);
    return false;
  }
}

async function stopEvdevPTT(): Promise<void> {
  if (!evdevListenerActive) return;

  try {
    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('stop_ptt');
  } catch {
    // ignore
  }

  if (evdevUnlisten) {
    evdevUnlisten();
    evdevUnlisten = null;
  }
  evdevListenerActive = false;
}

// --- Tauri global shortcut PTT (Windows/macOS) ---

// Convert our combo format ("Control+Space") to the plugin's format ("CommandOrControl+Space")
function comboToShortcutString(combo: string): string {
  return combo
    .split('+')
    .map((part) => {
      if (part === 'Control') return 'CommandOrControl';
      if (part === 'Meta') return 'Super';
      return part;
    })
    .join('+');
}

async function startGlobalShortcutPTT(combo: string): Promise<boolean> {
  if (!isTauriRuntime()) return false;

  try {
    const { register, unregister } = await import('@tauri-apps/plugin-global-shortcut');

    // Unregister previous shortcut if any
    if (globalShortcutRegistered) {
      try {
        await unregister(globalShortcutRegistered);
      } catch {
        // ignore
      }
    }

    const shortcut = comboToShortcutString(combo);
    await register(shortcut, (event) => {
      voiceManager.setPTTActive(event.state === 'Pressed');
    });

    globalShortcutRegistered = shortcut;
    return true;
  } catch (e) {
    console.warn('Global shortcut PTT not available:', e);
    return false;
  }
}

async function stopGlobalShortcutPTT(): Promise<void> {
  if (!globalShortcutRegistered) return;

  try {
    const { unregister } = await import('@tauri-apps/plugin-global-shortcut');
    await unregister(globalShortcutRegistered);
  } catch {
    // ignore
  }
  globalShortcutRegistered = null;
}

// --- Browser fallback (only works when app is focused) ---

// Parse a combo string into its modifier requirements and key code
function parseComboForBrowser(combo: string): { code: string; ctrl: boolean; shift: boolean; alt: boolean; meta: boolean } {
  const parts = combo.split('+');
  const mods = { ctrl: false, shift: false, alt: false, meta: false };
  for (const part of parts.slice(0, -1)) {
    if (part === 'Control') mods.ctrl = true;
    else if (part === 'Shift') mods.shift = true;
    else if (part === 'Alt') mods.alt = true;
    else if (part === 'Meta') mods.meta = true;
  }
  const keyPart = parts[parts.length - 1];
  return { code: codeFromTauriKey(keyPart), ...mods };
}

function browserModifiersMatch(e: KeyboardEvent): boolean {
  return (
    e.ctrlKey === browserPTTModifiers.ctrl &&
    e.shiftKey === browserPTTModifiers.shift &&
    e.altKey === browserPTTModifiers.alt &&
    e.metaKey === browserPTTModifiers.meta
  );
}

let browserPTTActive = false;

function onBrowserKeyDown(e: KeyboardEvent) {
  if (e.code === browserPTTKeyCode && browserModifiersMatch(e) && !e.repeat) {
    e.preventDefault();
    browserPTTActive = true;
    voiceManager.setPTTActive(true);
  }
}

function onBrowserKeyUp(e: KeyboardEvent) {
  if (!browserPTTActive) return;
  // Release when target key is released OR a required modifier is released
  if (e.code === browserPTTKeyCode || !browserModifiersMatch(e)) {
    e.preventDefault();
    browserPTTActive = false;
    voiceManager.setPTTActive(false);
  }
}

function registerBrowserPTT(combo: string): void {
  unregisterBrowserPTT();
  const parsed = parseComboForBrowser(combo);
  browserPTTKeyCode = parsed.code;
  browserPTTModifiers = { ctrl: parsed.ctrl, shift: parsed.shift, alt: parsed.alt, meta: parsed.meta };
  document.addEventListener('keydown', onBrowserKeyDown);
  document.addEventListener('keyup', onBrowserKeyUp);
  browserListenersActive = true;
}

function unregisterBrowserPTT(): void {
  if (!browserListenersActive) return;
  document.removeEventListener('keydown', onBrowserKeyDown);
  document.removeEventListener('keyup', onBrowserKeyUp);
  browserListenersActive = false;
}

// --- Public API ---

export async function registerPTTKey(key: string): Promise<boolean> {
  if (isTauriRuntime()) {
    // Try evdev first (Linux/Wayland global PTT)
    const evdevOk = await startEvdevPTT(key);
    if (evdevOk) {
      currentKey = key;
      registerBrowserPTT(key);
      return true;
    }

    // Try global shortcut plugin (Windows/macOS)
    const shortcutOk = await startGlobalShortcutPTT(key);
    if (shortcutOk) {
      currentKey = key;
      registerBrowserPTT(key);
      return true;
    }
  }

  // Browser-only or all native methods failed: use keyboard event fallback
  registerBrowserPTT(key);
  currentKey = key;
  return true;
}

export async function unregisterPTTKey(): Promise<void> {
  await stopEvdevPTT();
  await stopGlobalShortcutPTT();
  unregisterBrowserPTT();
  currentKey = null;
}

/** Human-readable label for a single key part */
function formatSingleKey(key: string): string {
  const labels: Record<string, string> = {
    Space: 'Space',
    Backquote: '` (Tilde)',
    CapsLock: 'Caps Lock',
    Tab: 'Tab',
    Control: 'Ctrl',
    Shift: 'Shift',
    Alt: 'Alt',
    Meta: 'Super',
  };
  return labels[key] || key;
}

/** Human-readable label for a PTT key combo */
export function formatKeyLabel(combo: string): string {
  return combo.split('+').map(formatSingleKey).join(' + ');
}

/** Convert a browser KeyboardEvent to a combo string for recording */
export function keyEventToTauriKey(e: KeyboardEvent): string | null {
  // Ignore modifier-only presses
  if (['Control', 'Shift', 'Alt', 'Meta'].includes(e.key)) return null;

  const parts: string[] = [];
  // Ordered: Control, Shift, Alt, Meta
  if (e.ctrlKey) parts.push('Control');
  if (e.shiftKey) parts.push('Shift');
  if (e.altKey) parts.push('Alt');
  if (e.metaKey) parts.push('Meta');
  parts.push(tauriKeyFromCode(e.code));
  return parts.join('+');
}

/**
 * Initialize PTT from saved settings.
 * Call this once on app startup after voice manager is ready.
 */
export async function initPTT(): Promise<VoiceSettings> {
  const settings = loadVoiceSettings();
  voiceManager.setInputMode(settings.inputMode);
  if (settings.inputMode === 'push-to-talk') {
    await registerPTTKey(settings.pttKey);
  }
  return settings;
}

/**
 * Switch voice input mode. Handles registration/unregistration of PTT key.
 */
export async function switchInputMode(mode: VoiceInputMode, pttKey: string): Promise<void> {
  voiceManager.setInputMode(mode);
  if (mode === 'push-to-talk') {
    await registerPTTKey(pttKey);
  } else {
    await unregisterPTTKey();
  }
  saveVoiceSettings({ inputMode: mode, pttKey });
}

/**
 * Change the PTT key binding. Only takes effect if currently in PTT mode.
 */
export async function changePTTKey(newKey: string): Promise<void> {
  const settings = loadVoiceSettings();
  settings.pttKey = newKey;
  saveVoiceSettings(settings);
  if (settings.inputMode === 'push-to-talk') {
    await registerPTTKey(newKey);
  }
}
