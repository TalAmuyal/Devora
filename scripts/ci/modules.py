"""Derives scripts/ci/modules.lock.json from scripts/ci/modules.toml and the build graphs (ADR-015).

`mise run ci-modules` writes the lock; `mise run ci-modules --check` fails, naming the difference, when the committed
lock isn't what this derives. Each module's inputs are its own paths, its hand-declared inputs, and what the graphs
show: the files its app bundle packs (the bundler's --list-sources), its mise tasks' `sources`, its crates' path
dependencies (Cargo, followed from `cargo metadata --no-deps`), and its Swift packages' local packages.
Every path is relative to the repository root, a directory ending in `/` and anything else one file, sorted, with any
path a listed directory already holds left out, so the selection (affected.sh) matches by prefix.
"""

import json
import os
import subprocess
import sys
import tomllib

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
MAP = os.path.join(ROOT, "scripts", "ci", "modules.toml")
LOCK = os.path.join(ROOT, "scripts", "ci", "modules.lock.json")
GLOB = set("*?[{")


def relative(path):
    """`path` (absolute) relative to the repository root, a directory with its trailing `/`."""
    rel = os.path.relpath(path, ROOT)
    if rel.startswith(".."):
        sys.exit(f"ci-modules: {path} is outside the repository")
    return rel + "/" if os.path.isdir(path) else rel


def widen(pattern, base):
    """A path or glob, relative to `base`, as the repository path that holds every file it can match."""
    parts = pattern.split("/")
    fixed = []
    for part in parts:
        if GLOB & set(part):
            break
        fixed.append(part)
    path = os.path.normpath(os.path.join(base, *fixed)) if fixed else os.path.normpath(base)
    if len(fixed) < len(parts):
        # The directory the glob starts in: it may match anything below.
        return relative(path) if os.path.isdir(path) else relative(os.path.dirname(path))
    return relative(path)


def run(*command, cwd=ROOT):
    return subprocess.run(command, cwd=cwd, check=True, capture_output=True, text=True).stdout


def bundle(app):
    script = os.path.join(ROOT, "bundler", "macos-ember", "populate-app-resources.sh")
    return [widen(line, ROOT) for line in run("bash", script, "--app", app, "--list-sources").split()]


def mise_sources(mise_file):
    """Every task's and dependency's `sources` in a mise.toml, relative to that file's directory."""
    base = os.path.dirname(os.path.join(ROOT, mise_file))
    with open(os.path.join(ROOT, mise_file), "rb") as f:
        config = tomllib.load(f)
    paths = []
    for section in ("tasks", "deps"):
        for entry in config.get(section, {}).values():
            if isinstance(entry, dict):
                paths += [widen(source, base) for source in entry.get("sources", [])]
    return paths


def crate_dirs(manifest, seen):
    """The crate at `manifest` and every crate it reaches through path dependencies, as their directories."""
    manifest = os.path.normpath(os.path.join(ROOT, manifest))
    if manifest in seen:
        return []
    seen.add(manifest)
    metadata = json.loads(run("cargo", "metadata", "--no-deps", "--format-version", "1", "--manifest-path", manifest))
    package = next(p for p in metadata["packages"] if os.path.normpath(p["manifest_path"]) == manifest)
    dirs = [relative(os.path.dirname(manifest))]
    for dependency in package["dependencies"]:
        if dependency.get("path"):
            dirs += crate_dirs(os.path.join(dependency["path"], "Cargo.toml"), seen)
    return dirs


def swift_packages(own):
    """The local packages a module's Swift packages depend on, from `swift package dump-package`."""
    dirs = []
    for path in own:
        package = os.path.join(ROOT, path)
        if not os.path.isfile(os.path.join(package, "Package.swift")):
            continue
        dump = json.loads(run("swift", "package", "dump-package", cwd=package))
        for dependency in dump.get("dependencies", []):
            for local in dependency.get("fileSystem", []):
                dirs.append(relative(os.path.normpath(os.path.join(package, local["path"]))))
    return dirs


def prune(paths):
    """Sorted, without duplicates, and without any path that a listed directory already holds."""
    kept = []
    for path in sorted(set(paths)):
        if not any(path != d and path.startswith(d) for d in paths if d.endswith("/")):
            kept.append(path)
    return kept


def derive():
    with open(MAP, "rb") as f:
        modules_map = tomllib.load(f)
    modules = {}
    for name, module in sorted(modules_map["modules"].items()):
        inputs = module["paths"] + module.get("inputs", [])
        if "bundle" in module:
            inputs += bundle(module["bundle"])
        if "mise" in module:
            inputs += mise_sources(module["mise"])
        seen = set()
        for manifest in module.get("crates", []):
            inputs += crate_dirs(manifest, seen)
        inputs += swift_packages(module["paths"])
        modules[name] = {"jobs": module["jobs"], "inputs": prune(inputs)}
    return {
        "nothing": {"paths": sorted(modules_map["nothing"]["paths"]), "names": sorted(modules_map["nothing"]["names"])},
        "everything": {
            "paths": sorted(modules_map["everything"]["paths"]),
            "manifests": sorted(modules_map["everything"]["manifests"]),
        },
        "modules": modules,
    }


def main():
    text = json.dumps(derive(), indent=2) + "\n"
    if sys.argv[1:] == ["--check"]:
        with open(LOCK) as f:
            committed = f.read()
        if committed != text:
            import difflib

            diff = difflib.unified_diff(
                committed.splitlines(), text.splitlines(), "committed modules.lock.json", "derived", lineterm=""
            )
            print("\n".join(diff))
            sys.exit("ci-modules: scripts/ci/modules.lock.json is stale: run `mise run ci-modules` and commit it")
        print("ci-modules: scripts/ci/modules.lock.json is current")
    else:
        with open(LOCK, "w") as f:
            f.write(text)
        print(f"ci-modules: wrote {relative(LOCK)}")


main()
