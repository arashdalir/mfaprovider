import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { App } from './App'
import { suppressBrowserMenu } from './lib/browser-menu'
import './index.css'

suppressBrowserMenu(document)

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
