import { createRoot } from 'react-dom/client'
import '../base.css'
import './launcher.css'
import { Launcher } from './Launcher'

createRoot(document.getElementById('root')!).render(<Launcher />)
