Bundler is a sub-project of the Devora project that is responsible for bundling the IDE into a self-contained app bundle for distribution.

For now, only MacOS is supported, so the bundler creates a MacOS app bundle that can be distributed and run on other machines without requiring users to set up the development environment manually.

Platform-specific files are under a platform-specific directory, e.g. `bundler/macos-ember/`.
This is done so more platforms can be added in the future without affecting the existing ones, and to keep platform-specific code organized and separate from the rest of the project.

`macos-ember/populate-app-resources.sh` is the single shared resource manifest for both apps. Its `--app` option selects plist identity; Ember is the default. `--list-sources` drives `bundle-fingerprint.sh --app <ember|sapphire>`; do not add a separate Sapphire resource list. Both apps currently use the existing Devora icon asset.

`macos-sapphire/bundle-sapphire.sh` copies the development app into `project-sapphire/build/bundle/Devora Sapphire.app`, populates it, sets its names and `INSTALL_SUFFIX`, and signs it last, with hardened runtime. `sapphire-install` copies that sealed app into the chosen directory; it never edits the signed contents afterward. CI passes `--prebuilt` to reuse the built app and Debi test-tools; it only builds the additional status-line/Judgess helpers. Shared source-build version naming is owned by `macos-ember/effective-version.sh`.

Both apps are signed by `sign-macos-app.sh`, as the last step of their bundle script: any later edit to a bundle breaks its seal, so `mac-install` and `sapphire-install` only move or copy the sealed app.
`macos-ember/bundle-ember.sh` takes `--name` and `--suffix` for that reason; without them (CD) it keeps the release name.
The identity is `DEVORA_SIGNING_IDENTITY` when set, else the self-signed "Devora Local Signing" when the keychain has it (`security find-identity`, not `-p codesigning`, which doesn't list it), else ad hoc with a warning.
A real identity makes the designated requirement "this bundle id, signed by this certificate", so macOS privacy permissions survive rebuilds, install suffixes and nightly updates; an ad hoc one is the build's hash, and every build is asked again.
CD imports the identity from the `DEVORA_SIGNING_P12` secrets into a throwaway keychain (`ci-signing-keychain.sh`), sets `DEVORA_SIGNING_IDENTITY` so a missing identity fails the build, and checks the app in each DMG with `verify-dmg-signature.sh`. PR CI has no secrets and stays ad hoc.

Both DMGs use their final app names (`Devora Ember.app` / `Devora Sapphire.app`) at the volume root; assets use hyphens (`Devora-Ember.dmg` / `Devora-Sapphire.dmg`). Ember retains the published `com.devora-org.devora` bundle id across its rename. Sapphire DMG creation lives in `macos-sapphire/create-dmg.sh` and shares `hdiutil-create-retry.sh`. CD has separate builds and one publish job; stable Sapphire is gated by the workflow's `SAPPHIRE_STABLE_ENABLED` switch. Installer transition behavior and isolated tests are documented in `docs/CD.md`.
