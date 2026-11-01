import { createRoot } from 'react-dom/client'
import '../base.css'
import './settings.css'
import { SettingsApp } from './SettingsApp'

createRoot(document.getElementById('root')!).render(<SettingsApp />)
