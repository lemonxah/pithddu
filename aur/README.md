# AUR packaging

Two packages for the [Arch User Repository](https://aur.archlinux.org):

| Package                  | Builds how                                              | Use when |
|--------------------------|---------------------------------------------------------|----------|
| `pithddu-dashboard-bin`  | Installs the prebuilt binary from the GitHub Release    | fastest; no compile |
| `pithddu-dashboard`      | Compiles from the tag + firmware submodule with `cargo` (Slint statically linked) | builds from source |

Each AUR package is its **own git repo** whose history is just `PKGBUILD` +
`.SRCINFO` (the AUR has one repo per `pkgbase`). The dirs here are the source of
truth; you copy them into the AUR repos when publishing.

## First-time setup (per machine)

1. Make an account at https://aur.archlinux.org and add your **SSH public key**
   under *My Account → SSH Public Key*.
2. (Optional) in `~/.ssh/config`:
   ```
   Host aur.archlinux.org
     User aur
     IdentityFile ~/.ssh/id_ed25519
   ```

## Create / publish a package

The AUR creates the repo on your **first push** of a new `pkgbase` — you don't
make it on the website first. Clone the (empty) repo by package name, drop the
files in, push:

```sh
git clone ssh://aur@aur.archlinux.org/pithddu-dashboard-bin.git
cd pithddu-dashboard-bin
cp /path/to/pithddu-dashboard/aur/pithddu-dashboard-bin/{PKGBUILD,.SRCINFO} .
git add PKGBUILD .SRCINFO
git commit -m "Initial import: pithddu-dashboard-bin 0.6.2"
git push
```

(`git clone` warning "you appear to have cloned an empty repository" is normal.)
Same steps for `pithddu-dashboard` against
`ssh://aur@aur.archlinux.org/pithddu-dashboard.git`.

## Updating to a new release

1. Edit the `PKGBUILD` here: bump `pkgver`, reset `pkgrel=1`.
2. For `-bin`, refresh the checksum:
   ```sh
   cd aur/pithddu-dashboard-bin && updpkgsums
   ```
3. Regenerate `.SRCINFO` (AUR rejects pushes whose `.SRCINFO` is stale):
   ```sh
   makepkg --printsrcinfo > .SRCINFO
   ```
4. Test locally: `makepkg -si` (build + install).
5. Copy `PKGBUILD` + `.SRCINFO` into the AUR clone, `commit`, `push`.

## Notes

- The **source** package (`pithddu-dashboard`) needs a tag (≥ `v0.7.0`) that
  includes the `install()`/CPack rules in `CMakeLists.txt` and the `LICENSE`
  file. Cut it with `just release` first.
- `pithddu-dashboard-bin` tracks the latest Release tarball and works against
  any tag, since `package()` re-points the binary's RPATH itself.
