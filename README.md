<div align="center">

<img src="resources/icon.png" width="96" alt="">

# Winkit

**Herramientas para Windows con Claude: escribe mejor, entiende otros idiomas y encuentra lo que buscas.**

Funciona en cualquier aplicación: Discord, Gmail, Word, Teams, el navegador… Vive en la bandeja del sistema y aparece con un atajo.

[![Release](https://img.shields.io/github/v/release/AleDev11/winkit?label=versi%C3%B3n)](https://github.com/AleDev11/winkit/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
![Windows](https://img.shields.io/badge/Windows-10%20%7C%2011-0078D4)

</div>

## Qué hace

### Buscador · `Alt+Espacio`

Una barra al estilo Spotlight:

- **Aplicaciones**: escribe «disc» y abre Discord. Busca en el menú Inicio, incluidas las apps de la Tienda.
- **Calculadora**: `15*3+7`, `340 + 15%`, `20% de 340`, `sqrt(16)`… `Enter` copia el resultado.
- **Web**: busca en Google, DuckDuckGo o Bing, o abre directamente una dirección (`github.com`).
- **Preguntar a Claude**: la respuesta aparece ahí mismo. `Ctrl+Enter` pregunta directamente.

### Escribir

Selecciona tu texto en cualquier app y:

| Atajo | Qué hace |
| --- | --- |
| `Ctrl+Alt+C` | Corrige ortografía y gramática, y reemplaza el texto |
| `Ctrl+Alt+T` | Traduce al idioma principal (inglés por defecto) y lo reemplaza. Si ya estaba en inglés, lo pasa al español |
| `Ctrl+Alt+X` | Menú con todo: corregir, traducir a cualquier idioma, mejorar la redacción, más formal, más cercano, más corto y tus acciones personalizadas |

En el menú verás los cambios marcados antes de aplicarlos: `Enter` reemplaza y `Esc` cierra sin tocar nada.

### Leer · `Ctrl+Alt+L`

Para entender lo que te escriben. Selecciona el mensaje y se abre una ventana con el original a un lado y la traducción al otro. No modifica nada de la app de origen. También puedes pegar cualquier texto en la ventana.

Todos los atajos, el modelo, los idiomas y el buscador se cambian en Ajustes (clic en el icono de la bandeja).

## Instalación

1. Descarga `winkit-setup-<versión>.exe` desde la [última versión](https://github.com/AleDev11/winkit/releases/latest) e instálalo.
2. Abre Winkit y pega tu [API key de Anthropic](https://console.anthropic.com/settings/keys) en Ajustes.

Se inicia sola con Windows y se queda en la bandeja del sistema.

> El instalador no está firmado, así que Windows SmartScreen puede avisar la primera vez: «Más información» → «Ejecutar de todas formas».

También puedes compilarlo tú:

```bash
git clone https://github.com/AleDev11/winkit.git
cd winkit
bun install
bun run dist       # deja el instalador en dist/
```

## Cómo funciona

- **Leer y reemplazar texto en otras apps.** Electron no puede leer los campos de texto de otras aplicaciones, así que Winkit usa el portapapeles: guarda lo que tenías copiado, simula `Ctrl+C`, envía el texto a Claude, pega el resultado con `Ctrl+V` y te devuelve tu portapapeles. Las teclas se simulan con `user32.dll` a través de [koffi](https://koffi.dev), sin compilar código nativo.
- **Privacidad.** Solo se envía texto a Anthropic cuando pulsas un atajo o preguntas a Claude. Winkit no escucha el teclado ni lee nada por su cuenta.
- **API key.** Se guarda cifrada con `safeStorage` (DPAPI) en `%APPDATA%\Winkit` y no sale del proceso principal. También se puede pasar con la variable de entorno `ANTHROPIC_API_KEY`.
- **Modelo.** Claude Haiku 5.5 por defecto, por velocidad. En Ajustes puedes elegir Sonnet 5.5 u Opus 5.5.

## Desarrollo

```bash
bun install
bun run dev        # app en modo desarrollo
bun run typecheck
bun test
bun run dist       # instalador NSIS en dist/
```

```
src/main/       proceso principal: bandeja, atajos, ventanas, portapapeles, Claude
src/preload/    puente IPC (window.winkit)
src/renderer/   viñeta (popup), lector (reader), buscador (launcher) y ajustes (settings), en React
src/shared/     tipos, ajustes por defecto y la calculadora
```

## Licencia

[MIT](LICENSE)
