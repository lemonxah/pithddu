# pithddu-dashboard tasks. Run `just` to list.
set shell := ["bash", "-uc"]

default:
    @just --list

# Build the SimHub plugin (net48) and install the DLL into the SimHub folder.
# Builds on Linux against a Wine SimHub prefix — no Windows/Mono needed. Override
# the SimHub path (or set $SIMHUB_PATH); the csproj copies the DLL in on success.
#   just simhub-plugin
#   just simhub-plugin "/path/to/SimHub"
simhub-plugin sh="":
    #!/usr/bin/env bash
    set -euo pipefail
    SH="{{sh}}"
    SH="${SH#sh=}"   # tolerate `just simhub-plugin sh=...` (just args are positional)
    SH="${SH:-${SIMHUB_PATH:-$HOME/linux-simracing-utils/pfx/drive_c/Program Files (x86)/SimHub}}"
    if [ ! -f "$SH/SimHub.Plugins.dll" ]; then
        echo "SimHub.Plugins.dll not found in: $SH" >&2
        echo "Pass the SimHub folder:  just simhub-plugin \"/path/to/SimHub\"" >&2
        exit 1
    fi
    echo "Building plugin against: $SH"
    dotnet build -c Release -p:SimHubPath="$SH" simhub-plugin/PithDdu.SimHubPlugin.csproj

# Cut a release by tagging + pushing (CI builds the bins and publishes the GitHub Release).
#   just release           -> bump the patch of the latest vX.Y.Z tag (0.1.0 if none)
#   just release 1.2.3     -> release exactly that version
release version="":
    #!/usr/bin/env bash
    set -euo pipefail
    git fetch --tags --quiet
    ver="{{version}}"
    if [ -z "$ver" ]; then
        last=$(git tag -l 'v*' --sort=-v:refname | head -n1)
        if [ -z "$last" ]; then
            ver="0.1.0"
        else
            base=${last#v}
            IFS='.' read -r MA MI PA <<<"$base"
            ver="${MA:-0}.${MI:-0}.$(( ${PA:-0} + 1 ))"
        fi
    fi
    ver=${ver#v}
    tag="v${ver}"
    if git rev-parse "$tag" >/dev/null 2>&1; then
        echo "tag $tag already exists — pick another version" >&2
        exit 1
    fi
    branch=$(git rev-parse --abbrev-ref HEAD)
    echo "Releasing $tag from $branch @ $(git rev-parse --short HEAD)"
    git tag -a "$tag" -m "release $tag"
    git push origin "$tag"
    echo "Pushed $tag — GitHub Actions will build and publish the release."
