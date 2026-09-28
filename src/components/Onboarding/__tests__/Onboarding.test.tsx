import React from 'react'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { Onboarding } from '../index'

const mockStore = {
  onboardingStep: 0,
  setOnboardingStep: vi.fn(),
  setOnboardingCompleted: vi.fn(),
  sttTestStatus: 'idle',
  llmTestStatus: 'idle',
  onboardingMode: 'byok',
  setOnboardingMode: vi.fn(),
  updateConfig: vi.fn(),
  config: {},
}

vi.mock('framer-motion', () => ({
  AnimatePresence: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  motion: {
    div: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  },
}))

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}))

vi.mock('../OnboardingLayout', () => ({
  OnboardingLayout: ({ children, onBack, onNext }: { children: React.ReactNode; onBack: () => void; onNext: () => void }) => (
    <div>
      <button type="button" onClick={onNext}>Next</button>
      <button type="button" onClick={onBack}>
        Back
      </button>
      {children}
    </div>
  ),
}))

vi.mock('../WelcomeStep', () => ({ WelcomeStep: () => <div>Welcome</div> }))
vi.mock('../AccountStep', () => ({ AccountStep: () => <div>Account</div> }))
vi.mock('../ModeSelectStep', () => ({ ModeSelectStep: () => <div>Mode</div> }))
vi.mock('../SttSetupStep', () => ({ SttSetupStep: () => <div>STT</div> }))
vi.mock('../LlmSetupStep', () => ({ LlmSetupStep: () => <div>LLM</div> }))
vi.mock('../PermissionsStep', () => ({ PermissionsStep: () => <div>Permissions</div> }))
vi.mock('../QuickTestStep', () => ({ QuickTestStep: () => <div>Quick Test</div> }))
vi.mock('../DoneStep', () => ({ DoneStep: () => <div>Done</div> }))

vi.mock('../../../stores/appStore', () => ({
  useAppStore: (selector: (state: typeof mockStore) => unknown) => selector(mockStore),
}))

vi.mock('../../../stores/authStore', () => ({
  useAuthStore: (selector: (state: { user: { id: string } }) => unknown) =>
    selector({ user: { id: 'test-user' } }),
}))

vi.mock('../../../lib/tauri', () => ({
  updateConfig: vi.fn().mockResolvedValue(undefined),
  saveOnboardingCompleted: vi.fn().mockResolvedValue(undefined),
}))

beforeEach(() => {
  mockStore.onboardingStep = 0
  mockStore.onboardingMode = 'byok'
  mockStore.setOnboardingStep.mockReset()
})

afterEach(() => cleanup())

describe('Personal BYOK onboarding', () => {
  it('moves directly from welcome to provider setup without account sign-in', async () => {
    render(<Onboarding />)
    fireEvent.click(screen.getByRole('button', { name: 'Next' }))
    await waitFor(() => expect(mockStore.setOnboardingStep).toHaveBeenCalledWith(1))
    cleanup()
    mockStore.onboardingStep = 1
    render(<Onboarding />)
    expect(screen.getByText('STT')).toBeInTheDocument()
    expect(screen.queryByText('Account')).not.toBeInTheDocument()
  })

  it('returns from LLM setup to STT setup', async () => {
    mockStore.onboardingStep = 2
    render(<Onboarding />)
    fireEvent.click(screen.getByRole('button', { name: 'Back' }))
    await waitFor(() => expect(mockStore.setOnboardingStep).toHaveBeenCalledWith(1))
  })
})
