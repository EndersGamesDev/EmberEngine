# Release signing keys

A release tag must be annotated and signed by a key represented by an armored public-key file in this directory. `.github/workflows/release.yml` imports this allow-list into a temporary keyring and fails before building or publishing when the tag signature does not verify, which binds release authority to reviewable repository history.

`wild-sky-maker.asc` carries fingerprint `309F3BF0ABAAC226494C8D9FFAB00BAD8B48D17C` and is the initial release key.

A second release key is added by a pull request that adds its armored public key as another `.asc` file in this directory. The pull request makes the new trust decision pass through the same integration record as workflow changes, while existing release tags and keys retain their original meaning.
