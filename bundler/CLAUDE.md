Bundler is a sub-project of the Devora project that is responsible for bundling the IDE into a self-contained app bundle for distribution.

For now, only MacOS is supported, so the bundler creates a MacOS app bundle that can be distributed and run on other machines without requiring users to set up the development environment manually.

Platform-specific files are under a platform-specific directory, e.g. `bundler/macos-ember/`.
This is done so more platforms can be added in the future without affecting the existing ones, and to keep platform-specific code organized and separate from the rest of the project.

`macos-ember/populate-app-resources.sh` is the single shared resource manifest for both apps. Its `--app` option selects plist identity; Ember is the default. `--list-sources` drives `bundle-fingerprint.sh --app <ember|sapphire>`; do not add a separate Sapphire resource list. Both apps currently use the existing Devora icon asset.

`macos-sapphire/bundle-sapphire.sh` copies the development app into `project-sapphire/build/bundle/Devora Sapphire.app`, populates it and signs every nested Mach-O before signing the app with hardened runtime and verifying deep/strict. Set names and `INSTALL_SUFFIX` before signing. `sapphire-install` copies that sealed app into the chosen directory; it never edits the signed contents afterward. CI passes `--prebuilt` to reuse the built app and Debi test-tools; it only builds the additional status-line/Judgess helpers. Shared source-build version naming is owned by `macos-ember/effective-version.sh`.
