# Releasing Escape

Releases are built by [`.github/workflows/release.yml`](../.github/workflows/release.yml)
when a `v*` tag is pushed. The workflow builds the NSIS installer, signs the update
and publishes a GitHub release with `latest.json`, which the app checks for updates.

## One-time setup

1. **Create the GitHub repository** and push the code.
2. **Point the updater at it.** In `src-tauri/tauri.conf.json` replace `OWNER` in
   `plugins.updater.endpoints` with your GitHub user or organisation:

   ```json
   "endpoints": ["https://github.com/<you>/escape/releases/latest/download/latest.json"]
   ```

   Until this is done the app never contacts GitHub — update checks are skipped.

3. **Add the signing secrets** (Settings → Secrets and variables → Actions):

   | Secret                               | Value                                                      |
   | ------------------------------------ | ---------------------------------------------------------- |
   | `TAURI_SIGNING_PRIVATE_KEY`          | contents of `%USERPROFILE%\.tauri\escape-updater.key`      |
   | `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | contents of `%USERPROFILE%\.tauri\escape-updater.password` |

   The public half is already in `tauri.conf.json` (`plugins.updater.pubkey`).
   **Back up both files**: without the private key no installed copy can be updated
   again. Never commit them.

## Publishing a version

1. Bump the version in `package.json`, `src-tauri/Cargo.toml` and
   `src-tauri/tauri.conf.json`, and add a section to `CHANGELOG.md`.
2. Commit, then tag and push:

   ```bash
   git tag v1.0.0
   git push origin main --tags
   ```

3. Wait for the **Release** workflow; the release appears under _Releases_.

## Testing an update (0.9.x → 1.0.0)

1. Set the version to `0.9.0`, tag `v0.9.0`, push, install that release.
2. Set the version back to `1.0.0`, tag `v1.0.0`, push.
3. In the installed 0.9.0: Настройки → «Проверить обновления» → «Обновить до 1.0.0».
   The app downloads, verifies the signature, installs and restarts.

## Building locally

```bash
npm run tauri build
```

With `createUpdaterArtifacts` on, the build signs the update and needs the key in the
environment (PowerShell):

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content "$env:USERPROFILE\.tauri\escape-updater.key" -Raw
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = Get-Content "$env:USERPROFILE\.tauri\escape-updater.password" -Raw
npm run tauri build
```

or skip update signing: `npm run tauri build -- --config src-tauri/tauri.ci.conf.json`.
