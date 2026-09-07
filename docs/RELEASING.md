# Releasing youTUIbe

The GitHub release is the source of truth. Tag a verified commit as `vX.Y.Z`, publish
the x86_64 Linux archive, then update the `youtuibe` and `youtuibe-bin` AUR repositories
with freshly generated `.SRCINFO` files and SHA-256 sums. The AUR repositories contain
only packaging metadata: source and release assets remain on GitHub.

`youtuibe` builds from the signed/tagged source archive. `youtuibe-bin` installs the
matching prebuilt GitHub release archive. Both packages provide `youtuibe`, so they must
not be installed together.
