Bundler is a sub-project of the Devora project that is responsible for bundling the IDE into a self-contained app bundle for distribution.

For now, only MacOS is supported, so the bundler creates a MacOS app bundle that can be distributed and run on other machines without requiring users to set up the development environment manually.

Platform-specific files are under a platform-specific directory, e.g. `bundler/macos/`.
This is done so more platforms can be added in the future without affecting the existing ones, and to keep platform-specific code organized and separate from the rest of the project.
