<div align="center">

<img src="resources/icon.png" width="96" alt="">

# Winkit

**Herramientas para Windows con Claude: escribe mejor, entiende otros idiomas, encuentra lo que buscas y elige colores.**

Funciona en cualquier aplicación: Discord, Gmail, Word, Teams, el navegador… Vive en la bandeja del sistema y aparece con un atajo. Hecho en Rust con [Tauri](https://tauri.app): ligero y rápido.

[![Release](https://img.shields.io/github/v/release/AleDev11/winkit?label=versi%C3%B3n)](https://github.com/AleDev11/winkit/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
![Windows](https://img.shields.io/badge/Windows-10%20%7C%2011-0078D4)

</div>

## Qué hace

### Paleta · `Ctrl+Alt+X`

Selecciona texto en cualquier app y pulsa el atajo: aparece una paleta junto al cursor con todo lo que puedes hacer con él.

- **Corregir**, **Traducir**, **Mejorar**, **Más formal**, **Más cercano**, **Más corto** y tus acciones personalizadas. Pulsa el número o escribe para filtrar.
- **Instrucción libre**: escribe lo que quieras («tono de disculpa», «en tres viñetas») y pulsa `Enter`.
- **Entender**: abre un panel con el original a un lado y la traducción al otro. No toca la app de origen.
- Verás los cambios marcados antes de aplicarlos: `Enter` reemplaza, `Ctrl+C` copia, `Ctrl+R` lo vuelve a generar y `Esc` cierra sin tocar nada.

**Traducciones con dirección automática.** Winkit detecta el idioma del texto en tu equipo, sin conexión, antes de llamar a Claude. Si escribes en tu idioma, lo traduce a tu idioma de destino (inglés por defecto), y si te escriben en otro, a tu idioma. En la paleta verás `Inglés → Español` desde el primer momento.

### Atajos directos

| Atajo | Qué hace |
| --- | --- |
| `Ctrl+Alt+C` | Corrige la selección y la reemplaza |
| `Ctrl+Alt+T` | Traduce la selección (dirección automática) y la reemplaza |
| `Ctrl+Alt+L` | Abre «Entender» con la selección (o vacío, para pegar texto) |

### Buscador · `Alt+Espacio`

Una barra al estilo Spotlight:

- **Aplicaciones**: escribe «disc» y abre Discord. Incluye las apps de la Tienda.
- **Calculadora**: `15*3+7`, `340 + 15%`, `sqrt(16)`… `Enter` copia el resultado.
- **Unidades**: `10 km a mi`, `5 kg en lb`, `100 f a c`, `2 gb a mb`.
- **Comandos**: bloquear, suspender, apagar, reiniciar, vaciar la papelera, abrir Descargas o la configuración de Windows…
- **Web**: busca en Google, DuckDuckGo o Bing, o abre directamente una dirección (`github.com`).
- **Preguntar a Claude**: la respuesta aparece ahí mismo. `Ctrl+Enter` pregunta directamente.

### Selector de color · `Ctrl+Alt+P`

Congela la pantalla y abre una lupa. Haz clic sobre cualquier píxel y el color se copia en HEX, RGB o HSL (lo eliges en Ajustes). Las flechas mueven la lupa píxel a píxel (`Mayús` para ir de 10 en 10) y `Esc` cancela.

### Clips · `Ctrl+Alt+G`

Guarda los últimos minutos de tu pantalla en un MP4, como la repetición instantánea de NVIDIA. Mientras está activo, Winkit graba sin parar en un búfer temporal que se va borrando, y al pulsar el atajo guarda lo último (de 1 a 30 minutos, tú eliges) en la carpeta que quieras (`Vídeos\Winkit` por defecto).

- Codifica con la GPU (NVIDIA NVENC, AMD AMF o Intel Quick Sync) y apenas usa CPU.
- Audio del sistema y micrófono en pistas separadas.
- 30 o 60 fps y tres niveles de calidad.
- Viene desactivado: se activa en Ajustes → Clips. Nada sale de tu PC.

Todos los atajos, el modelo, los idiomas y el resto se cambian en Ajustes (clic en el icono de la bandeja).

## Instalación

1. Descarga `Winkit_<versión>_x64-setup.exe` desde la [última versión](https://github.com/AleDev11/winkit/releases/latest) e instálalo.
2. Abre Winkit y pega tu [API key de Anthropic](https://console.anthropic.com/settings/keys) en Ajustes.

Se inicia con Windows, se queda en la bandeja del sistema y **se actualiza sola**: cuando sale una versión nueva la descarga en segundo plano y la instala al reiniciar.

Si vienes de la versión anterior (Electron), tus ajustes y tu API key se conservan. Esa versión no se actualiza sola a esta: instala esta una vez y desinstala la antigua desde Configuración → Aplicaciones.

> El instalador no está firmado, así que Windows SmartScreen puede avisar la primera vez: «Más información» → «Ejecutar de todas formas».

## Cómo funciona

- **Leer y reemplazar texto en otras apps.** Winkit usa el portapapeles: guarda lo que tenías copiado (todos los formatos), simula `Ctrl+C`, envía el texto a Claude, pega el resultado con `Ctrl+V` y te devuelve tu portapapeles. El texto temporal se marca para que no aparezca en el historial de `Win+V`.
- **Privacidad.** Solo se envía texto a Anthropic cuando pulsas un atajo o preguntas a Claude. La detección de idioma es local. Winkit no escucha el teclado ni lee nada por su cuenta.
- **API key.** Se guarda cifrada con DPAPI en `%APPDATA%\Winkit` y no sale del proceso de Rust. También se puede pasar con la variable de entorno `ANTHROPIC_API_KEY`.
- **Modelo.** Claude Haiku 5.5 por defecto, por velocidad. En Ajustes puedes elegir Sonnet 5.5 u Opus 5.5.

## Desarrollo

Necesitas [Rust](https://rustup.rs), Node.js y las Build Tools de Visual Studio (C++).

```bash
npm install
npm run dev        # app en modo desarrollo
npm run typecheck
npm test           # tests de la interfaz (vitest)
cd src-tauri && cargo test
npm run build      # instalador NSIS en src-tauri/target/release/bundle/nsis/
```

### Publicar una versión

1. Sube la versión en `package.json` (Tauri la lee de ahí).
2. `git tag v0.3.0 && git push origin v0.3.0`.

El workflow `.github/workflows/release.yml` compila el instalador, lo firma y publica la release con `latest.json`, que es lo que consultan las copias instaladas. Necesita dos secretos en el repositorio: `TAURI_SIGNING_PRIVATE_KEY` (el contenido de la clave privada de firma) y `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (vacío si no tiene contraseña). Si se pierde la clave privada, las copias instaladas dejan de poder actualizarse.

```
src-tauri/src/        backend en Rust
  replay/             clips: captura, codificación por GPU, audio, búfer y remux
  updater.rs          actualizaciones automáticas
  palette.rs          captura de la selección, paleta y reemplazo
  claude.rs           Messages API con streaming
  lang.rs             detección de idioma local
  launcher.rs         buscador
  picker.rs           selector de color
  win/                API de Windows: teclado, portapapeles, DPAPI, pantalla, apps, sistema
src/windows/          ventanas en React: palette, launcher, settings, picker
src/shared/           contrato de tipos, calculadora, unidades, comandos, diff
src/lib/api.ts        puente con Rust (invoke y eventos)
```

## Licencia

[MIT](LICENSE)
