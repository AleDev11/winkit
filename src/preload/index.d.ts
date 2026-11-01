import type { WinkitApi } from './index'

declare global {
  interface Window {
    winkit: WinkitApi
  }
}
