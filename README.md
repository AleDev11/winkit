<div align="center">

<img src="resources/icon.png" width="96" alt="">

# Winkit

**Herramientas para Windows con IA: escribe mejor, entiende otros idiomas, encuentra lo que buscas, guarda clips de tu pantalla y elige colores.**

Funciona en cualquier aplicación: Discord, Gmail, Word, Teams, el navegador… Vive en la bandeja del sistema y aparece con un atajo. Hecho en Rust con [Tauri](https://tauri.app): el instalador pesa unos 3 MB y apenas consume.

Usa la IA que prefieras: **Claude, OpenAI, DeepSeek, Gemini, OpenRouter**, tu propio servidor o una **IA local** gratis y sin conexión.

[![Release](https://img.shields.io/github/v/release/AleDev11/winkit?label=versi%C3%B3n)](https://github.com/AleDev11/winkit/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
![Windows](https://img.shields.io/badge/Windows-10%20%7C%2011-0078D4)
![Rust](https://img.shields.io/badge/Rust-Tauri%202-CE422B)

</div>

## Qué hace

### Paleta · `Ctrl+Alt+X`

Selecciona texto en cualquier app y pulsa el atajo: aparece una paleta junto al cursor con todo lo que puedes hacer con él.

- **Corregir**, **Traducir**, **Mejorar**, **Más formal**, **Más cercano**, **Más corto** y tus acciones personalizadas. Pulsa el número o escribe para filtrar.
- **Instrucción libre**: escribe lo que quieras («tono de disculpa», «en tres viñetas») y pulsa `Enter`.
- Verás los cambios marcados antes de aplicarlos: `Enter` reemplaza, `Ctrl+C` copia, `Ctrl+R` lo vuelve a generar y `Esc` cierra sin tocar nada. El altavoz lo lee en voz alta.

**Traducciones con dirección automática.** Winkit detecta el idioma del texto en tu equipo, sin conexión, antes de llamar a la IA. Si escribes en tu idioma, lo traduce a tu idioma de destino (inglés por defecto), y si te escriben en otro, a tu idioma. En la paleta verás `Inglés → Español` desde el primer momento.

### Entender · `Ctrl+Alt+L`

Para lo que te escriben: el original a un lado y la traducción al otro, sin tocar la app de origen. Es una ventana normal: la mueves, le cambias el tamaño (se adapta, hasta ponerse una cosa encima de la otra) y recuerda el tamaño. **Leer** te la lee en voz alta.

### Atajos directos

| Atajo | Qué hace |
| --- | --- |
| `Ctrl+Alt+C` | Corrige la selección y la reemplaza |
| `Ctrl+Alt+T` | Traduce la selección (dirección automática) y la reemplaza |

### Buscador · `Alt+Espacio`

Una barra al estilo Spotlight:

- **Aplicaciones**: escribe «disc» y abre Discord. Incluye las apps de la Tienda.
- **Calculadora**: `15*3+7`, `340 + 15%`, `sqrt(16)`… `Enter` copia el resultado.
- **Unidades**: `10 km a mi`, `5 kg en lb`, `100 f a c`, `2 gb a mb`.
- **Comandos**: bloquear, suspender, apagar, reiniciar, vaciar la papelera, abrir Descargas o la configuración de Windows…
- **Web**: busca en Google, DuckDuckGo o Bing, o abre directamente una dirección (`github.com`).
- **Preguntar a la IA**: la respuesta aparece ahí mismo. `Ctrl+Enter` pregunta directamente.

### Clips · `Ctrl+Alt+G`

Guarda los últimos minutos de tu pantalla en un MP4, como la repetición instantánea de NVIDIA. Mientras está activo, Winkit graba sin parar en un búfer temporal que se va borrando, y al pulsar el atajo guarda lo último (de 1 a 30 minutos, tú eliges) en la carpeta que quieras (`Vídeos\Winkit` por defecto).

- Codifica con la GPU (NVIDIA NVENC, AMD AMF o Intel Quick Sync) y apenas usa CPU.
- Eliges el monitor, la salida de audio y el micrófono (con medidor de nivel para saber cuál es cuál). Audio del sistema y micrófono van en pistas separadas.
- 30 o 60 fps, tres niveles de calidad y HDR convertido a SDR para que se vea como en pantalla.
- Viene desactivado: se activa en Ajustes → Clips. Nada sale de tu PC.

### Selector de color · `Ctrl+Alt+P`

Congela la pantalla y abre una lupa. Haz clic sobre cualquier píxel y el color se copia en HEX, RGB o HSL (lo eliges en Ajustes). Las flechas mueven la lupa píxel a píxel (`Mayús` para ir de 10 en 10) y `Esc` cancela.

### Avisos

«Clip guardado», «Color copiado», un error… salen en una esquina (la eliges en Ajustes, o junto al ratón), con botones para lo siguiente («Ver clip», «Reintentar») y una cuenta atrás que se pausa al pasar el ratón.

Todos los atajos, la IA, la voz, los idiomas y el resto se cambian en Ajustes (clic en el icono de la bandeja).

## La IA que tú elijas

En Ajustes → IA eliges un modelo por defecto y, si quieres, uno distinto para cada cosa: escribir, entender, el buscador o el asistente.

| Proveedor | Qué necesitas | Coste |
| --- | --- | --- |
| **Claude** (Anthropic) | [API key](https://console.anthropic.com/settings/keys) | De pago por uso |
| **OpenAI** | [API key](https://platform.openai.com/api-keys) | De pago por uso |
| **DeepSeek** | [API key](https://platform.deepseek.com/api_keys) | De pago por uso, de los más baratos |
| **Google Gemini** | [API key](https://aistudio.google.com/apikey) | Con nivel gratuito limitado |
| **OpenRouter** | [API key](https://openrouter.ai/keys) | Cientos de modelos con una sola key |
| **Servidor propio** | La dirección de cualquier API compatible con OpenAI (LM Studio, Ollama, vLLM…) | Lo que cueste tu servidor |
| **IA local** | Nada: Winkit la descarga | **Gratis y sin conexión** |

Los modelos se consultan en directo a cada proveedor, con una nota de precio. Cada key se guarda cifrada con DPAPI (solo tu usuario de Windows puede leerla).

### IA local

Al activarla, Winkit descarga el motor [llama.cpp](https://github.com/ggml-org/llama.cpp) (~40 MB, con aceleración por GPU vía Vulkan y CPU de respaldo) y el modelo que elijas (2,8–7 GB). Quien no la activa no descarga nada. Todo va con versión fija y se comprueba con SHA-256 antes de usarse.

| Modelo | Descarga | Para qué |
| --- | --- | --- |
| **Gemma 4 E2B** (recomendado) | 2,8 GB | Rápido y ligero; corrige y traduce bien en español |
| Gemma 4 E4B | 4,6 GB | Más fino con textos largos |
| Qwen 3.5 4B | 3,0 GB | Instrucciones libres y código |
| Gemma 4 12B | 7,0 GB | El más capaz; pide una GPU con 8 GB o más |

Se carga al usarla y **se descarga de la memoria tras unos minutos sin uso**, para no quitarte VRAM mientras juegas. En una RTX 3070, Gemma 4 E2B corrige una frase en ~0,3 s a ~100 tokens/s.

## Voz

En Ajustes → Voz eliges con qué se leen en voz alta las traducciones, los resultados y las respuestas:

| Motor | Coste | Notas |
| --- | --- | --- |
| **Voces de Windows** | Gratis | Sin descargas; dependen de las voces que tengas instaladas en Windows |
| **Voz local** ([Supertonic 3](https://github.com/k2-fsa/sherpa-onnx)) | Gratis, sin conexión | Voces neuronales en 31 idiomas, español incluido; se descarga al activarla (~140 MB) |
| **ElevenLabs** | De pago (~0,04–0,08 $ por 1.000 caracteres) | La mejor calidad; con tu API key |
| **OpenAI** | De pago | Usa la key de OpenAI de la página IA |

## Instalación

1. Descarga `Winkit_<versión>_x64-setup.exe` desde la [última versión](https://github.com/AleDev11/winkit/releases/latest) e instálalo.
2. En Ajustes → IA, pega la API key de tu proveedor o activa la IA local.

Se inicia con Windows, se queda en la bandeja del sistema y **se actualiza sola**: cuando sale una versión nueva la descarga en segundo plano y la instala al reiniciar.

Si vienes de la versión anterior (Electron), tus ajustes y tu API key se conservan. Esa versión no se actualiza sola a esta: instala esta una vez y desinstala la antigua desde Configuración → Aplicaciones.

> El instalador no está firmado, así que Windows SmartScreen puede avisar la primera vez: «Más información» → «Ejecutar de todas formas».

## Cómo funciona

- **Leer y reemplazar texto en otras apps.** Winkit usa el portapapeles: guarda lo que tenías copiado (todos los formatos), simula `Ctrl+C`, envía el texto a la IA, pega el resultado con `Ctrl+V` y te devuelve tu portapapeles. El texto temporal se marca para que no aparezca en el historial de `Win+V`.
- **Privacidad.** El texto solo sale de tu PC cuando pulsas un atajo o preguntas algo, y solo hacia el proveedor que hayas elegido. Con la IA local y la voz local no sale nada. La detección de idioma es local. Winkit no escucha el teclado ni lee nada por su cuenta.
- **API keys.** Se guardan cifradas con DPAPI en `%APPDATA%\Winkit` y no salen del proceso de Rust. También se pueden pasar con variables de entorno (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `DEEPSEEK_API_KEY`, `GEMINI_API_KEY`, `OPENROUTER_API_KEY`, `ELEVENLABS_API_KEY`).
- **Registro.** Si algo falla, el motivo queda en `%LOCALAPPDATA%\Winkit\winkit.log` (sin tus textos ni tus keys).

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

**Diseñar sin Tauri.** Con `npm run vite`, abre cualquier ventana en el navegador (por ejemplo `http://localhost:1420/windows/settings/index.html`) y funcionará con datos de ejemplo realistas (`src/dev/mock.ts`). La paleta acepta `?mode=menu|run|understand|notice`. Ese código no entra en la versión publicada.

**Pruebas reales.** Las que graban la pantalla, descargan modelos o llaman a las APIs están marcadas con `#[ignore]`; se lanzan a mano, por ejemplo `cargo test --lib -- --ignored --nocapture --test-threads=1 replay::tests`.

### Publicar una versión

1. Sube la versión en `package.json` (Tauri la lee de ahí) y en `src-tauri/Cargo.toml`.
2. `git tag v0.3.1 && git push origin v0.3.1`.

El workflow `.github/workflows/release.yml` pasa los tests, compila Winkit, monta el instalador con Inno Setup, lo firma y publica la release con `latest.json`, que es lo que consultan las copias instaladas. Necesita el secreto `TAURI_SIGNING_PRIVATE_KEY` (el contenido de la clave privada de firma) y, si la clave tiene contraseña, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. Si se pierde la clave privada, las copias instaladas dejan de poder actualizarse.

### Instalador

El instalador está hecho con [Inno Setup](https://jrsoftware.org/isinfo.php) (6.7 o posterior), pero no usa su asistente: la ventana es propia, sin bordes y con las esquinas redondeadas de Windows 11, y todo se dibuja con GDI+ con los mismos tokens de Fluent que la app. Coge el acento y el modo claro u oscuro de Windows.

```
npm run tauri build -- --no-bundle
iscc installer\winkit.iss
```

El resultado queda en `installer/Output/`. Para ver las pantallas sin instalar nada, compila con `/DPREVIEW` y abre el `.exe` con `/DEMO=welcome` (o `options`, `installing`, `update`, `done`, `cancel`, `error`), y opcionalmente `/THEME=light|dark` y `/ACCENT=0078D4`.

El actualizador de la app lo lanza con `/P /UPDATE /R`: entonces se salta las pantallas, instala con la ventana de progreso y vuelve a abrir Winkit. Al instalar encima de una versión que vino con el instalador NSIS anterior, quita su entrada de «Aplicaciones» para que Winkit no salga dos veces.

```
src-tauri/src/        backend en Rust
  ai/                 proveedores de IA (Anthropic, compatibles con OpenAI, Gemini) y prompts
    local/            IA local: catálogo, descargas verificadas, servidor llama.cpp, GPU
  voice/              voz: Windows, sherpa-onnx (local), ElevenLabs, OpenAI y reproductor WASAPI
  replay/             clips: captura, codificación por GPU, audio, búfer, remux y HDR
  palette.rs          captura de la selección, paleta, «Entender» y avisos
  launcher.rs         buscador
  picker.rs           selector de color
  lang.rs             detección de idioma local
  updater.rs          actualizaciones automáticas
  log.rs              registro en archivo
  win/                API de Windows: teclado, portapapeles, DPAPI, pantalla, apps, sistema
src/windows/          ventanas en React: palette, launcher, settings, picker
src/components/       piezas comunes (desplegable, botón de leer, logo…)
src/shared/           contrato de tipos, calculadora, unidades, comandos, diff
src/lib/api.ts        puente con Rust (invoke y eventos)
src/dev/mock.ts       datos de ejemplo para diseñar en el navegador
installer/            instalador de Inno Setup: winkit.iss (qué instala) y ui.iss (la interfaz)
```

## Licencia

[MIT](LICENSE)
