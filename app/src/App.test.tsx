import { render, screen } from '@testing-library/react'
import { expect, it } from 'vitest'
import { ApiContext } from '@/ipc/api-context'
import { fakeApi } from '@/test/fake-api'
import { App } from './App'

it('renders the app title and the unlock screen for an existing vault', async () => {
  render(
    <ApiContext.Provider value={fakeApi()}>
      <App />
    </ApiContext.Provider>,
  )
  expect(screen.getByRole('heading', { name: 'MFA Provider' })).toBeInTheDocument()
  expect(await screen.findByRole('heading', { name: 'Unlock' })).toBeInTheDocument()
})
