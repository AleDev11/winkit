import { describe, expect, test } from 'bun:test'
import type { AppEntry } from '@shared/types'
import { looksLikeUrl, matchApps } from './search'

const apps: AppEntry[] = ['Discord', 'Visual Studio Code', 'Configuración', 'Spotify', 'Steam', 'Docker Desktop'].map((name) => ({
  id: name,
  name,
  kind: 'shortcut'
}))

describe('matchApps', () => {
  test('prefijo primero', () => {
    expect(matchApps(apps, 'disc')[0].name).toBe('Discord')
    expect(matchApps(apps, 'st').map((a) => a.name)).toEqual(['Steam', 'Visual Studio Code'])
  })
  test('sin tildes', () => {
    expect(matchApps(apps, 'configuracion')[0].name).toBe('Configuración')
  })
  test('iniciales y letras sueltas', () => {
    expect(matchApps(apps, 'vsc')[0].name).toBe('Visual Studio Code')
  })
  test('nada que ver', () => {
    expect(matchApps(apps, 'zzz')).toEqual([])
  })
})

describe('looksLikeUrl', () => {
  test.each(['github.com', 'https://claude.ai/new', 'localhost:5173', 'es.wikipedia.org/wiki/Tilde'])('%s es URL', (q) => {
    expect(looksLikeUrl(q)).toBe(true)
  })
  test.each(['hola mundo', 'discord', '2+2', 'qué es node.js'])('%s no es URL', (q) => {
    expect(looksLikeUrl(q)).toBe(false)
  })
})
