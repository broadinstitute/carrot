#!/usr/bin/env bash

current_version="0.6.3"
new_version=$current_version

__set_new_version()
{
  # Split version into major, minor, and patch
  IFS="." read -a version_array <<< "$current_version"
  major=$((version_array[0]))
  minor=$((version_array[1]))
  patch=$((version_array[2]))
  # Get version we're bumping from args and change version accordingly
  case $1 in
    "major")
      major=$((major + 1))
      minor=0
      patch=0
      ;;
    "minor")
      minor=$((minor + 1))
      patch=0
      ;;
    "patch")
      patch=$((patch + 1))
      ;;
    *)
      echo "Usage: ./bumpversion.sh major|minor|patch"
      exit 1
      ;;
  esac

  new_version="$major.$minor.$patch"
}

set -euo pipefail

__set_new_version $1

# OSX and Linux sed are different, so we need to account for that with regard to the -i option
os_string=$(uname)
case $os_string in
  "Darwin")
    # Replace only the first instance in Cargo.toml (because there might be others that are for other libraries)
    sed -i '' -e "1,6s#version = \"$current_version\"#version = \"$new_version\"#" Cargo.toml
    # Update in the user guide
    sed -i '' -e "s#v$current_version#v$new_version#" UserGuide.md
    # Do it in the places we need to update in carrot_cli
    sed -i '' -e "s#version = \"$current_version\"#version = \"$new_version\"#" carrot_cli/setup.py
    sed -i '' -e "s#Current version: $current_version#Current version: $new_version#" carrot_cli/README.md
    sed -i '' -e "s#__version = \"$current_version\"#__version = \"$new_version\"#" carrot_cli/src/carrot_cli/__main__.py
    # Finally update it in this script
    sed -i '' -e "s#current_version=\"$current_version\"#current_version=\"$new_version\"#" bumpversion.sh
    ;;
  "Linux")
    # Replace only the first instance in Cargo.toml (because there might be others that are for other libraries)
    sed -i -e "1,6s#version = \"$current_version\"#version = \"$new_version\"#" Cargo.toml
    # Update in the user guide
    sed -i -e "s#v$current_version#v$new_version#" UserGuide.md
    # Do it in the places we need to update in carrot_cli
    sed -i -e "s#version = \"$current_version\"#version = \"$new_version\"#" carrot_cli/setup.py
    sed -i -e "s#Current version: $current_version#Current version: $new_version#" carrot_cli/README.md
    sed -i -e "s#__version = \"$current_version\"#__version = \"$new_version\"#" carrot_cli/src/carrot_cli/__main__.py
    # Finally update it in this script
    sed -i -e "s#current_version=\"$current_version\"#current_version=\"$new_version\"#" bumpversion.sh
    ;;
  *)
    echo "Unsupported operating system"
    exit 1
    ;;
esac
