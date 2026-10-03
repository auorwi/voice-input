import { useCallback, useEffect, useRef, useState } from 'react'
import { listen } from '@tauri-apps/api/event'
import { Inbox } from 'lucide-react'
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
  const [drafts, setDrafts] = useState<Record<string, string>>({})
  const [feedback, setFeedback] = useState<{
    sessionId: string
    text: string
    message: string
  } | null>(null)
  const [busy, setBusy] = useState(false)
  const [loadError, setLoadError] = useState(false)
  const [hasLoaded, setHasLoaded] = useState(false)
  const latestRefresh = useRef(0)
  const selected = results.find((item) => item.sessionId === selectedId) ?? results[0]
  const selectedText = selected ? (drafts[selected.sessionId] ?? selected.text) : ''
  const visibleFeedback =
    feedback && feedback.sessionId === selected?.sessionId && feedback.text === selectedText
      ? feedback.message
      : ''

  const refresh = useCallback(async () => {
    const request = ++latestRefresh.current
    try {
      const pending = await listPendingDictationResults()
      if (request !== latestRefresh.current) return
      setResults(pending)
      setDrafts((current) =>
        Object.fromEntries(
          Object.entries(current).filter(([id]) => pending.some((item) => item.sessionId === id)),
        ),
      )
      setSelectedId(
        (id) =>
          pending.find((item) => item.sessionId === id)?.sessionId ?? pending[0]?.sessionId ?? null,
      )
      setLoadError(false)
      setHasLoaded(true)
    } catch {
      if (request === latestRefresh.current) setLoadError(true)
    }
  }, [])

  useEffect(() => {
    let active = true
    let unlisten: (() => void) | undefined
    void listen('dictation-result:changed', () => {
      if (active) void refresh()
    })
      .then((dispose) => {
        if (active) unlisten = dispose
        else dispose()
        if (active) void refresh()
      })
      .catch(() => {
        if (active) void refresh()
      })
    return () => {
      active = false
      latestRefresh.current += 1
      unlisten?.()
    }
  }, [refresh])

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.isComposing || event.keyCode === 229) return
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
    setFeedback(null)
    const text = selectedText
    try {
      await copyPendingDictationResult(selected.sessionId, text)
      setFeedback({ sessionId: selected.sessionId, text, message: '已复制' })
    } catch {
      setFeedback({
        sessionId: selected.sessionId,
        text,
        message: '复制失败，请选中文字手动复制。',
      })
    } finally {
      setBusy(false)
    }
  }

  async function closeSelected() {
    if (!selected || busy) return
    setBusy(true)
    try {
      await dismissPendingDictationResult(selected.sessionId)
      setFeedback(null)
      await refresh()
    } catch {
      setFeedback({
        sessionId: selected.sessionId,
        text: selectedText,
        message: '关闭失败，请重试。',
      })
    } finally {
      setBusy(false)
    }
  }

  return (
    <main className="flex h-screen flex-col gap-4 overflow-hidden bg-bg-primary p-5 text-text-primary">
      <header className="flex items-center justify-between">
        <h1 className="text-lg font-semibold">语音输入结果</h1>
        <span className="text-xs text-text-tertiary">
          {results.length > 1 ? `${results.length} 条待处理` : ''}
        </span>
      </header>
      {loadError && <p role="alert">无法读取结果，请稍后重试。</p>}
      {results.length > 1 && (
        <nav className="flex gap-2 overflow-x-auto" aria-label="待处理结果">
          {results.map((item, index) => (
            <button
              key={item.sessionId}
              type="button"
              aria-pressed={item.sessionId === selected?.sessionId}
              onClick={() => {
                setSelectedId(item.sessionId)
                setFeedback(null)
              }}
              className="max-w-40 truncate rounded-lg border border-border px-3 py-1 text-sm"
            >
              {index + 1} {item.text.slice(0, 16)}
            </button>
          ))}
        </nav>
      )}
      {hasLoaded && !loadError && results.length === 0 && (
        <div className="flex flex-1 flex-col items-center justify-center gap-2 text-center">
          <Inbox aria-hidden size={32} className="text-text-tertiary" />
          <p className="text-sm text-text-secondary">暂无待处理结果</p>
          <p className="text-xs text-text-tertiary">当语音输入无法自动填入时，文字会保留在这里。</p>
        </div>
      )}
      {selected && (
        <>
          <p className="text-sm text-text-secondary">
            {reasons[selected.reason] ?? reasons.unknown_target}
          </p>
          <p id="edit-result-hint" className="text-xs text-text-tertiary">
            可直接修改下方文字，再复制到需要的位置。
          </p>
          <textarea
            aria-label="完整识别文字"
            aria-describedby="edit-result-hint"
            className="min-h-0 flex-1 resize-none rounded-xl border border-border bg-bg-secondary p-4 text-base leading-relaxed text-text-primary"
            value={selectedText}
            onChange={(event) => {
              const text = event.target.value
              setDrafts((current) => ({ ...current, [selected.sessionId]: text }))
              setFeedback(null)
            }}
          />
          <div className="flex items-center justify-between gap-3">
            <span role="status" className="text-sm text-text-secondary">
              {visibleFeedback}
            </span>
            <div className="flex gap-2">
              <button
                type="button"
                disabled={busy}
                onClick={closeSelected}
                className="rounded-lg border border-border px-4 py-2"
              >
                关闭
              </button>
              <button
                type="button"
                disabled={busy || !selectedText.length}
                onClick={copySelected}
                className="rounded-lg bg-accent px-5 py-2 font-medium text-white"
              >
                复制
              </button>
            </div>
          </div>
        </>
      )}
    </main>
  )
}
