# Fire Racer v1 contract test

This directory holds `hosted_contract.rs`, which replays fixture suite `fire-v1-hosted-contract` against manifest slot `fire/1`.

The Fire Racer v1 package and evergreen-server registry depend on this gate to preserve the frozen racing wire and session behavior; never edit its expectations by hand, and use the [parent contract record](../README.md) and [hosted-version rules](../../../../docs/one-server-evergreen.md) when cutting a successor.
