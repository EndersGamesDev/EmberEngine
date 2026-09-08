# Arena v5 contract test

This directory holds `hosted_contract.rs`, which replays fixture suite `arena-v5-hosted-contract` against manifest slot `arena/5`.

The Arena v5 package and evergreen-server registry depend on this gate to preserve the frozen wire and session behavior; never edit its expectations by hand, and use the [parent contract record](../README.md) and [hosted-version rules](../../../../docs/one-server-evergreen.md) when cutting a successor.
