import { createRoot } from 'react-dom/client'
import '../base.css'
import './reader.css'
import { Reader } from './Reader'

createRoot(document.getElementById('root')!).render(<Reader />)
