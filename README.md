# Escape

**Dynamic Island для Windows.** Компактная «пилюля» по центру верхнего края экрана показывает живые события — музыку, время, заряд, — а при наведении раскрывается в панель с плеером, историей буфера обмена и командной строкой.

> Статус: ранняя разработка (этап 0 — каркас). План и архитектура — в [PLAN.md](PLAN.md).

## Возможности (план v1.0)

- Пилюля с часами, батареей и мини-индикатором музыки; плавные пружинные анимации.
- Клики проходят сквозь пустые области, фокус не отнимается, в полноэкранных приложениях остров прячется.
- Плеер для любого приложения через системные медиа-элементы Windows (Spotify, браузеры и т.д.).
- История буфера обмена (Ctrl+Alt+V) с поиском, закреплением и вставкой; пароли из менеджеров не сохраняются.
- Командная строка (Alt+Space): программы, приложения Store, калькулятор, поиск в браузере.

## Стек

Tauri 2 (WebView2) · Rust · React 19 + TypeScript · Vite · Tailwind CSS v4 · motion · zustand.

## Как запустить

### Требования

- Windows 10 22H2+ или Windows 11
- [Rust](https://rustup.rs/) (toolchain `stable-x86_64-pc-windows-msvc`)
- [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) с компонентом «Desktop development with C++»
- Node.js 20+
- WebView2 Runtime (в Windows 11 уже есть)

### Разработка

```bash
npm install
npm run tauri dev
```

### Сборка установщика

```bash
npm run tauri build
```

Установщик NSIS появится в `src-tauri/target/release/bundle/nsis/`.

### Проверки

```bash
npm run lint && npm run typecheck && npm test
cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

## Лицензия

MIT
