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
  const promise = new Promise<T>((finish) => {
    resolve = finish
  })
  return { promise, resolve }
}

beforeEach(() => {
  native.invoke.mockReset()
  native.hide.mockClear()
  native.listeners.clear()
})
afterEach(cleanup)

describe('DictationResult', () => {
  it('shows an empty state once loading finishes with no pending results', async () => {
    native.invoke.mockImplementation(async (name: string) =>
      name === 'list_pending_dictation_results' ? [] : undefined,
    )
    render(<DictationResult />)
    expect(await screen.findByText('暂无待处理结果')).toBeInTheDocument()
    expect(screen.queryByRole('textbox')).not.toBeInTheDocument()
  })

  it('does not show the empty state before the first load resolves', async () => {
    const pending = deferred<(typeof first)[]>()
    native.invoke.mockImplementation(async (name: string) =>
      name === 'list_pending_dictation_results' ? pending.promise : undefined,
    )
    render(<DictationResult />)
    expect(screen.queryByText('暂无待处理结果')).not.toBeInTheDocument()
    await act(async () => {
      pending.resolve([])
      await pending.promise
    })
    expect(await screen.findByText('暂无待处理结果')).toBeInTheDocument()
  })

  it('lets the user edit multiline text and copies exactly the edited draft', async () => {
    native.invoke.mockImplementation(async (name: string) =>
      name === 'list_pending_dictation_results' ? [first] : undefined,
    )
    render(<DictationResult />)
    const editor = await screen.findByRole('textbox', { name: '完整识别文字' })
    expect(editor).not.toHaveAttribute('readonly')
    fireEvent.change(editor, { target: { value: '修正后的文字\n第二行 🎙️' } })
    fireEvent.click(screen.getByRole('button', { name: '复制' }))
    await waitFor(() =>
      expect(native.invoke).toHaveBeenCalledWith('copy_pending_dictation_result', {
        sessionId: first.sessionId,
        editedText: '修正后的文字\n第二行 🎙️',
      }),
    )
    expect(await screen.findByText('已复制')).toBeInTheDocument()
  })

  it('preserves independent drafts when new recordings arrive and selection changes', async () => {
    let results = [first]
    native.invoke.mockImplementation(async (name: string) =>
      name === 'list_pending_dictation_results' ? results : undefined,
    )
    render(<DictationResult />)
    const editor = await screen.findByRole('textbox')
    fireEvent.change(editor, { target: { value: '第一条的草稿' } })
    results = [first, second]
    await act(async () => {
      native.listeners.get('dictation-result:changed')?.({ payload: null })
    })
    expect(screen.getByRole('textbox')).toHaveValue('第一条的草稿')
    fireEvent.click(screen.getByRole('button', { name: /2.*第二段/ }))
    fireEvent.change(screen.getByRole('textbox'), { target: { value: '第二条的草稿' } })
    fireEvent.click(screen.getByRole('button', { name: /1.*第一段/ }))
    expect(screen.getByRole('textbox')).toHaveValue('第一条的草稿')
    fireEvent.click(screen.getByRole('button', { name: /2.*第二段/ }))
    expect(screen.getByRole('textbox')).toHaveValue('第二条的草稿')
  })

  it('does not claim a changed draft was copied when an earlier copy resolves', async () => {
    const copying = deferred<void>()
    native.invoke.mockImplementation(async (name: string) => {
      if (name === 'list_pending_dictation_results') return [first, second]
      if (name === 'copy_pending_dictation_result') return copying.promise
    })
    render(<DictationResult />)
    await screen.findByDisplayValue(first.text)
    fireEvent.click(screen.getByRole('button', { name: '复制' }))
    fireEvent.change(screen.getByRole('textbox'), { target: { value: '复制后又改过' } })
    await act(async () => {
      copying.resolve()
      await copying.promise
    })
    expect(screen.queryByText('已复制')).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: /2.*第二段/ }))
    expect(screen.queryByText('已复制')).not.toBeInTheDocument()
  })

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
    fireEvent.change(screen.getByRole('textbox'), { target: { value: '失败也保留修改后的文字' } })
    fireEvent.click(screen.getByRole('button', { name: '复制' }))
    expect(await screen.findByText('复制失败，请选中文字手动复制。')).toBeInTheDocument()
    expect(screen.getByRole('textbox')).toHaveValue('失败也保留修改后的文字')
    fireEvent.click(screen.getByRole('button', { name: '复制' }))
    expect(await screen.findByText('已复制')).toBeInTheDocument()
    expect(native.invoke).toHaveBeenLastCalledWith('copy_pending_dictation_result', {
      sessionId: first.sessionId,
      editedText: '失败也保留修改后的文字',
    })
    expect(native.hide).not.toHaveBeenCalled()
  })

  it('keeps an empty draft editable and disables copying until text is entered', async () => {
    native.invoke.mockImplementation(async (name: string) =>
      name === 'list_pending_dictation_results' ? [first] : undefined,
    )
    render(<DictationResult />)
    const editor = await screen.findByRole('textbox')
    fireEvent.change(editor, { target: { value: '' } })
    expect(editor).toHaveValue('')
    expect(screen.getByRole('button', { name: '复制' })).toBeDisabled()
    fireEvent.change(editor, { target: { value: '重新输入' } })
    expect(screen.getByRole('button', { name: '复制' })).toBeEnabled()
  })

  it('does not dismiss the editor when Escape cancels Chinese input composition', async () => {
    native.invoke.mockImplementation(async (name: string) =>
      name === 'list_pending_dictation_results' ? [first] : undefined,
    )
    render(<DictationResult />)
    const editor = await screen.findByRole('textbox')
    fireEvent.keyDown(editor, { key: 'Escape', isComposing: true })
    fireEvent.keyDown(editor, { key: 'Escape', keyCode: 229 })
    expect(native.invoke).not.toHaveBeenCalledWith(
      'dismiss_pending_dictation_result',
      expect.anything(),
    )
    expect(editor).toBeInTheDocument()
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
    const oldList = deferred<(typeof first)[]>()
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
    await act(async () => {
      native.listeners.get('dictation-result:changed')?.({ payload: null })
    })
    expect(await screen.findByDisplayValue(second.text)).toBeInTheDocument()
    await act(async () => {
      oldList.resolve([])
      await oldList.promise
    })
    expect(screen.getByDisplayValue(second.text)).toBeInTheDocument()
    expect(native.hide).not.toHaveBeenCalled()
  })

  it('discards a stale list while dismissal and a newer publish overlap', async () => {
    const dismissal = deferred<number>()
    const oldList = deferred<(typeof first)[]>()
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
    await act(async () => {
      native.listeners.get('dictation-result:changed')?.({ payload: null })
    })
    await waitFor(() => expect(listCount).toBe(2))
    await act(async () => {
      dismissal.resolve(1)
      await dismissal.promise
    })
    expect(await screen.findByDisplayValue(second.text)).toBeInTheDocument()
    await act(async () => {
      oldList.resolve([first])
      await oldList.promise
    })
    expect(screen.getByDisplayValue(second.text)).toBeInTheDocument()
    expect(native.hide).not.toHaveBeenCalled()
  })
})
