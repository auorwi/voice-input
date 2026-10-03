import React from 'react'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { useAppStore, type HistoryEntry } from '../../../stores/appStore'
import { addCorrectionRule, clearHistory, getCorrectionRules } from '../../../lib/tauri'
import { History } from '../index'

vi.mock('framer-motion', () => ({
  AnimatePresence: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  motion: new Proxy(
    {},
    {
      get:
        (_target, tag: string) =>
        ({ children, ...props }: React.HTMLAttributes<HTMLElement>) =>
          React.createElement(tag, props, children),
    },
  ),
}))

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}))

vi.mock('../../../lib/tauri', () => ({
  addCorrectionRule: vi.fn().mockResolvedValue(undefined),
  clearHistory: vi.fn().mockResolvedValue(undefined),
  getCorrectionRules: vi.fn().mockResolvedValue([]),
}))

const entry: HistoryEntry = {
  id: 1,
  created_at: new Date().toISOString(),
  context_profile_id: 'chat.slack',
  context_label: 'Slack',
  context_icon_key: 'slack',
  context_family: 'work_chat',
  browser_access_status: 'not_applicable',
  provider_kind: 'local',
  raw_text: 'open type less',
  polished_text: 'OpenTypeless',
  language: 'en',
  duration_ms: 1000,
  active_scene_id: null,
  active_scene_source: null,
  active_scene_name: null,
  active_scene_prompt_chars: null,
  active_scene_prompt_truncated: false,
  output_status: null,
  output_error: null,
}

describe('History correction creation', () => {
  beforeEach(() => {
    useAppStore.setState({ history: [entry], correctionRules: [] })
    vi.clearAllMocks()
    vi.mocked(addCorrectionRule).mockResolvedValue(undefined)
    vi.mocked(getCorrectionRules).mockResolvedValue([
      { id: 2, pattern: 'open type less', replacement: 'OpenTypeless', enabled: true },
    ])
  })

  afterEach(() => {
    cleanup()
    useAppStore.setState(useAppStore.getInitialState())
  })

  it('does not infer a correction until the user explicitly opens and saves it', async () => {
    render(<History />)
    expect(addCorrectionRule).not.toHaveBeenCalled()

    const menuButton = screen.getByRole('button', { name: 'history.moreActions' })
    fireEvent.click(menuButton)
    fireEvent.click(screen.getByRole('menuitem', { name: 'history.createCorrection' }))

    const dialog = screen.getByRole('dialog', { name: 'history.createCorrection' })
    expect(dialog).toBeInTheDocument()
    expect(screen.getByLabelText('dictionary.wrongPhrase')).toHaveValue('open type less')
    expect(screen.getByLabelText('dictionary.correctPhrase')).toHaveValue('OpenTypeless')
    expect(addCorrectionRule).not.toHaveBeenCalled()

    fireEvent.click(screen.getByRole('button', { name: 'history.saveCorrection' }))
    await waitFor(() => {
      expect(addCorrectionRule).toHaveBeenCalledWith('open type less', 'OpenTypeless')
      expect(getCorrectionRules).toHaveBeenCalled()
      expect(useAppStore.getState().correctionRules).toHaveLength(1)
    })
  })

  it('leaves fields empty instead of truncating an overlong history source', () => {
    useAppStore.setState({
      history: [{ ...entry, raw_text: 'x'.repeat(121), polished_text: 'y'.repeat(121) }],
    })
    render(<History />)

    fireEvent.click(screen.getByRole('button', { name: 'history.moreActions' }))
    fireEvent.click(screen.getByRole('menuitem', { name: 'history.createCorrection' }))

    expect(screen.getByLabelText('dictionary.wrongPhrase')).toHaveValue('')
    expect(screen.getByLabelText('dictionary.correctPhrase')).toHaveValue('')
    expect(screen.getByLabelText('dictionary.wrongPhrase')).toHaveFocus()
  })

  it('closes the row menu on Escape and restores its trigger focus', async () => {
    render(<History />)
    const menuButton = screen.getByRole('button', { name: 'history.moreActions' })

    fireEvent.click(menuButton)
    expect(screen.getByRole('menu')).toBeInTheDocument()
    fireEvent.keyDown(window, { key: 'Escape' })

    expect(screen.queryByRole('menu')).toBeNull()
    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'history.moreActions' })).toHaveFocus(),
    )
  })

  it('clears history through an in-app confirmation instead of window.confirm', async () => {
    const confirmSpy = vi.spyOn(window, 'confirm')
    render(<History />)

    fireEvent.click(screen.getByRole('button', { name: 'history.clearAll' }))
    expect(confirmSpy).not.toHaveBeenCalled()
    expect(screen.getByText('history.clearConfirm')).toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: 'history.confirmClear' }))

    await waitFor(() => {
      expect(clearHistory).toHaveBeenCalled()
      expect(useAppStore.getState().history).toEqual([])
    })
    confirmSpy.mockRestore()
  })
})

describe('History search filtering', () => {
  const englishEntry: HistoryEntry = {
    ...entry,
    id: 2,
    raw_text: 'hello world',
    polished_text: 'Hello World',
  }
  const chineseEntry: HistoryEntry = {
    ...entry,
    id: 3,
    context_label: '微信',
    raw_text: '你好世界',
    polished_text: '你好世界',
  }

  const searchInput = () => screen.getByPlaceholderText('history.searchPlaceholder')

  beforeEach(() => {
    useAppStore.setState({ history: [englishEntry, chineseEntry], correctionRules: [] })
    vi.clearAllMocks()
  })

  afterEach(() => {
    cleanup()
    useAppStore.setState(useAppStore.getInitialState())
  })

  it.each(['hello', 'HELLO', 'Hello'])('matches English regardless of case: %s', (query) => {
    render(<History />)
    fireEvent.change(searchInput(), { target: { value: query } })
    expect(screen.getByText('Hello World')).toBeInTheDocument()
    expect(screen.queryByText('你好世界')).toBeNull()
  })

  it('matches Chinese keywords and excludes non-matching entries', () => {
    render(<History />)
    fireEvent.change(searchInput(), { target: { value: '世界' } })
    expect(screen.getByText('你好世界')).toBeInTheDocument()
    expect(screen.queryByText('Hello World')).toBeNull()
  })

  it('shows all entries when the search term is empty', () => {
    render(<History />)
    expect(screen.getByText('Hello World')).toBeInTheDocument()
    expect(screen.getByText('你好世界')).toBeInTheDocument()
  })

  it('shows the no-results message when nothing matches', () => {
    render(<History />)
    fireEvent.change(searchInput(), { target: { value: 'zzz' } })
    expect(screen.getByText('history.noResults')).toBeInTheDocument()
    expect(screen.queryByText('Hello World')).toBeNull()
    expect(screen.queryByText('你好世界')).toBeNull()
  })

  it('shows the clear button only for a non-empty search and restores the full list on click', () => {
    render(<History />)
    expect(screen.queryByRole('button', { name: 'history.clearSearch' })).toBeNull()

    fireEvent.change(searchInput(), { target: { value: 'hello' } })
    expect(screen.queryByText('你好世界')).toBeNull()

    fireEvent.click(screen.getByRole('button', { name: 'history.clearSearch' }))
    expect(searchInput()).toHaveValue('')
    expect(screen.queryByRole('button', { name: 'history.clearSearch' })).toBeNull()
    expect(screen.getByText('Hello World')).toBeInTheDocument()
    expect(screen.getByText('你好世界')).toBeInTheDocument()
    expect(clearHistory).not.toHaveBeenCalled()
  })

  it('restores the full list after clearing a search with no matches', () => {
    render(<History />)
    fireEvent.change(searchInput(), { target: { value: 'zzz' } })
    expect(screen.getByText('history.noResults')).toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: 'history.clearSearch' }))
    expect(searchInput()).toHaveValue('')
    expect(screen.getByText('Hello World')).toBeInTheDocument()
    expect(screen.getByText('你好世界')).toBeInTheDocument()
  })

  it('treats a whitespace-only search as non-empty and can clear it', () => {
    render(<History />)
    fireEvent.change(searchInput(), { target: { value: ' ' } })
    expect(screen.getByRole('button', { name: 'history.clearSearch' })).toBeInTheDocument()
    expect(screen.getByText('Hello World')).toBeInTheDocument()
    expect(screen.queryByText('你好世界')).toBeNull()

    fireEvent.click(screen.getByRole('button', { name: 'history.clearSearch' }))
    expect(searchInput()).toHaveValue('')
    expect(screen.getByText('Hello World')).toBeInTheDocument()
    expect(screen.getByText('你好世界')).toBeInTheDocument()
  })
})
