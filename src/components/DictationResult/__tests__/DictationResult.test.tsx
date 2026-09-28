import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { DictationResult } from '../DictationResult'

const native = vi.hoisted(() => {
  const listeners = new Map<string, (event: { payload: unknown }) => void>()
  return {
    invoke: vi.fn(),
    hide: vi.fn().mockResolvedValue(undefined),
    listeners,
    listen: vi.fn(async (name: string, callback: (event: { payload: unknown }) => void) => {
      listeners.set(name, callback)
      return () => listeners.delete(name)
    }),
  }
})

vi.mock('@tauri-apps/api/core', () => ({ invoke: native.invoke }))
vi.mock('@tauri-apps/api/event', () => ({ listen: native.listen }))
vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({ hide: native.hide }),
}))

const first = { sessionId: 'recording-1', text: '第一段完整文字。', reason: 'no_target' }
const second = { sessionId: 'recording-2', text: '第二段完整文字。', reason: 'target_changed' }

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((finish) => { resolve = finish })
  return { promise, resolve }
}

beforeEach(() => {
  native.invoke.mockReset()
  native.hide.mockClear()
  native.listeners.clear()
})
afterEach(cleanup)

describe('DictationResult', () => {
  it('retrieves a result that arrived before the renderer loaded', async () => {
    native.invoke.mockImplementation(async (name: string) =>
      name === 'list_pending_dictation_results' ? [first] : undefined,
    )
    render(<DictationResult />)
    expect(await screen.findByDisplayValue('第一段完整文字。')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '复制' })).toBeInTheDocument()
    expect(native.hide).not.toHaveBeenCalled()
  })

  it('keeps selectable text visible after copy fails and reports success after retry', async () => {
    let copyAttempts = 0
    native.invoke.mockImplementation(async (name: string) => {
      if (name === 'list_pending_dictation_results') return [first]
      if (name === 'copy_pending_dictation_result') {
        copyAttempts += 1
        if (copyAttempts === 1) throw new Error('clipboard unavailable')
      }
    })
    render(<DictationResult />)
    await screen.findByDisplayValue(first.text)
    fireEvent.click(screen.getByRole('button', { name: '复制' }))
    expect(await screen.findByText('复制失败，请选中文字手动复制。')).toBeInTheDocument()
    expect(screen.getByDisplayValue(first.text)).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: '复制' }))
    expect(await screen.findByText('已复制')).toBeInTheDocument()
    expect(native.hide).not.toHaveBeenCalled()
  })

  it('navigates consecutive results and closes each explicit session separately', async () => {
    let results = [first, second]
    native.invoke.mockImplementation(async (name: string, args?: { sessionId: string }) => {
      if (name === 'list_pending_dictation_results') return [...results]
      if (name === 'dismiss_pending_dictation_result') {
        results = results.filter((item) => item.sessionId !== args?.sessionId)
        return results.length
      }
    })
    render(<DictationResult />)
    await screen.findByDisplayValue(first.text)
    fireEvent.click(screen.getByRole('button', { name: /2.*第二段/ }))
    expect(screen.getByDisplayValue(second.text)).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: '关闭' }))
    await waitFor(() => expect(screen.getByDisplayValue(first.text)).toBeInTheDocument())
    expect(native.invoke).toHaveBeenCalledWith('dismiss_pending_dictation_result', {
      sessionId: 'recording-2',
    })
    expect(native.hide).not.toHaveBeenCalled()
    fireEvent.click(screen.getByRole('button', { name: '关闭' }))
    await waitFor(() => expect(screen.queryByLabelText('完整识别文字')).not.toBeInTheDocument())
    expect(native.hide).not.toHaveBeenCalled()
  })

  it('keeps a newer result visible when an older close list resolves empty last', async () => {
    const oldList = deferred<typeof first[]>()
    let listCount = 0
    native.invoke.mockImplementation(async (name: string) => {
      if (name === 'dismiss_pending_dictation_result') return 0
      if (name === 'list_pending_dictation_results') {
        listCount += 1
        if (listCount === 1) return [first]
        if (listCount === 2) return oldList.promise
        return [second]
      }
    })
    render(<DictationResult />)
    await screen.findByDisplayValue(first.text)
    fireEvent.click(screen.getByRole('button', { name: '关闭' }))
    await waitFor(() => expect(listCount).toBe(2))
    await act(async () => { native.listeners.get('dictation-result:changed')?.({ payload: null }) })
    expect(await screen.findByDisplayValue(second.text)).toBeInTheDocument()
    await act(async () => { oldList.resolve([]); await oldList.promise })
    expect(screen.getByDisplayValue(second.text)).toBeInTheDocument()
    expect(native.hide).not.toHaveBeenCalled()
  })

  it('discards a stale list while dismissal and a newer publish overlap', async () => {
    const dismissal = deferred<number>()
    const oldList = deferred<typeof first[]>()
    let listCount = 0
    native.invoke.mockImplementation(async (name: string) => {
      if (name === 'dismiss_pending_dictation_result') return dismissal.promise
      if (name === 'list_pending_dictation_results') {
        listCount += 1
        if (listCount === 1) return [first]
        if (listCount === 2) return oldList.promise
        return [second]
      }
    })
    render(<DictationResult />)
    await screen.findByDisplayValue(first.text)
    fireEvent.click(screen.getByRole('button', { name: '关闭' }))
    await act(async () => { native.listeners.get('dictation-result:changed')?.({ payload: null }) })
    await waitFor(() => expect(listCount).toBe(2))
    await act(async () => { dismissal.resolve(1); await dismissal.promise })
    expect(await screen.findByDisplayValue(second.text)).toBeInTheDocument()
    await act(async () => { oldList.resolve([first]); await oldList.promise })
    expect(screen.getByDisplayValue(second.text)).toBeInTheDocument()
    expect(native.hide).not.toHaveBeenCalled()
  })
})
