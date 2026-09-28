import { useCallback, useEffect, useState } from 'react'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import {
  copyPendingDictationResult,
  dismissPendingDictationResult,
  listPendingDictationResults,
} from '../../lib/tauri'
import type { PendingDictationResult } from '../../lib/tauri'

const reasons: Record<string, string> = {
  no_target: '未找到可输入的位置，文字已保留在这里。',
  target_changed: '输入位置已改变，文字已保留在这里。',
  unknown_target: '无法确认输入位置，文字已保留在这里。',
  output_failed: '自动输入未完成，请从这里复制文字。',
  llm_raw_fallback: '润色暂不可用，以下是原始转写。',
  copy_only: '文字已复制到剪贴板，也保留在这里。',
}

export function DictationResult() {
  const [results, setResults] = useState<PendingDictationResult[]>([])
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [feedback, setFeedback] = useState('')
  const [busy, setBusy] = useState(false)
  const [loadError, setLoadError] = useState(false)
  const selected = results.find((item) => item.sessionId === selectedId) ?? results[0]

  const refresh = useCallback(async () => {
    try {
      const pending = await listPendingDictationResults()
      setResults(pending)
      setSelectedId((id) => pending.find((item) => item.sessionId === id)?.sessionId ?? pending[0]?.sessionId ?? null)
      setLoadError(false)
      return pending
    } catch {
      setLoadError(true)
      return null
    }
  }, [])

  useEffect(() => {
    let active = true
    let unlisten: (() => void) | undefined
    void listen('dictation-result:changed', () => {
      if (active) void refresh()
    }).then((dispose) => {
      if (active) unlisten = dispose
      else dispose()
      if (active) void refresh()
    }).catch(() => {
      if (active) void refresh()
    })
    return () => {
      active = false
      unlisten?.()
    }
  }, [refresh])

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && selected) {
        event.preventDefault()
        void closeSelected()
      }
    }
    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
    // closeSelected uses the current selected session; re-register on change.
  })

  async function copySelected() {
    if (!selected || busy) return
    setBusy(true)
    setFeedback('')
    try {
      await copyPendingDictationResult(selected.sessionId)
      setFeedback('已复制')
    } catch {
      setFeedback('复制失败，请选中文字手动复制。')
    } finally {
      setBusy(false)
    }
  }

  async function closeSelected() {
    if (!selected || busy) return
    setBusy(true)
    try {
      await dismissPendingDictationResult(selected.sessionId)
      setFeedback('')
      const pending = await refresh()
      if (pending?.length === 0) await getCurrentWindow().hide()
    } catch {
      setFeedback('关闭失败，请重试。')
    } finally {
      setBusy(false)
    }
  }

  return (
    <main className="flex h-screen flex-col gap-4 overflow-hidden bg-bg-primary p-5 text-text-primary">
      <header className="flex items-center justify-between">
        <h1 className="text-lg font-semibold">语音输入结果</h1>
        <span className="text-xs text-text-tertiary">{results.length > 1 ? `${results.length} 条待处理` : ''}</span>
      </header>
      {loadError && <p role="alert">无法读取结果，请稍后重试。</p>}
      {results.length > 1 && (
        <nav className="flex gap-2 overflow-x-auto" aria-label="待处理结果">
          {results.map((item, index) => (
            <button
              key={item.sessionId}
              type="button"
              aria-pressed={item.sessionId === selected?.sessionId}
              onClick={() => { setSelectedId(item.sessionId); setFeedback('') }}
              className="max-w-40 truncate rounded-lg border border-border px-3 py-1 text-sm"
            >
              {index + 1} {item.text.slice(0, 16)}
            </button>
          ))}
        </nav>
      )}
      {selected && (
        <>
          <p className="text-sm text-text-secondary">{reasons[selected.reason] ?? reasons.unknown_target}</p>
          <textarea
            aria-label="完整识别文字"
            className="min-h-0 flex-1 resize-none rounded-xl border border-border bg-bg-secondary p-4 text-base leading-relaxed text-text-primary"
            value={selected.text}
            readOnly
            onFocus={(event) => event.currentTarget.select()}
          />
          <div className="flex items-center justify-between gap-3">
            <span role="status" className="text-sm text-text-secondary">{feedback}</span>
            <div className="flex gap-2">
              <button type="button" disabled={busy} onClick={closeSelected} className="rounded-lg border border-border px-4 py-2">关闭</button>
              <button type="button" disabled={busy} onClick={copySelected} className="rounded-lg bg-accent px-5 py-2 font-medium text-white">复制</button>
            </div>
          </div>
        </>
      )}
    </main>
  )
}
