# Releasing Forte

Releases are built by `.github/workflows/release.yml` when a `v*.*.*` tag is
pushed. It creates a **draft** GitHub Release with installers for macOS,
Windows and Linux; publish it by hand after checking it.

```sh
git tag v0.2.0
git push origin v0.2.0
```

Bump `version` in `package.json`, `src-tauri/Cargo.toml` and
`src-tauri/tauri.conf.json` first.

## Turning on automatic updates (one-time)

The app already contains the updater, but it stays off until releases are
signed. The private key is yours alone: never commit it.

1. Create a key pair (choose a password when asked):

   ```sh
   npx tauri signer generate -w ~/.tauri/forte.key
   ```

2. In GitHub → Settings → Secrets and variables → Actions, add:
   - `TAURI_SIGNING_PRIVATE_KEY`: the contents of `~/.tauri/forte.key`
   - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: the password from step 1

3. In `src-tauri/tauri.conf.json`, add the **public** key (the contents of
   `~/.tauri/forte.key.pub`) and turn on update bundles:

   ```json
   "bundle": {
     "createUpdaterArtifacts": true,
     ...
   },
   "plugins": {
     "updater": {
       "pubkey": "<contents of forte.key.pub>",
       "endpoints": [
         "https://github.com/jmishra01/forte/releases/latest/download/latest.json"
       ]
     }
   }
   ```

4. Tag a release as above and publish the draft. The workflow uploads
   `latest.json`, which installed copies check on startup and through the
   "Check for updates" command.

Keep a backup of the private key: without it, existing installs can't
accept new updates.
