# Repository ruleset payloads

These files are complete request bodies for GitHub's repository-ruleset REST endpoints, so the reviewable policy and the externally applied rules can be compared byte for byte. Each body is accepted by `gh api --method POST repos/EndersGamesDev/EmberEngine/rulesets --input FILE`; the same body is accepted by the corresponding `PUT` endpoint when an existing ruleset is replaced.

`develop.json` replaces ruleset `22736795` with `gh api --method PUT repos/EndersGamesDev/EmberEngine/rulesets/22736795 --input deploy/rulesets/develop.json`. `main-authorization.json` replaces ruleset `22736890` after `main` exists. `branch-names.json` records live ruleset `22770318`, which admits new branch names only under the documented integration, release, feature and lane patterns without changing existing refs. The other payloads remain separate because integrity rules carry no bypass actor while authorization rules grant only the narrowly required creation or update path.

Branch commit signatures are not required for now. Release provenance remains signed because `release.yml` verifies annotated release tags against `deploy/keys/`; a later branch-signature policy can add `required_signatures` to both `develop.json` and `main-integrity.json` without changing the authorization payloads.

The JSON files contain no repository identifier or credential. Applying them is an external repository-administration operation, and drift between the active settings and these bodies is checked by comparing the API response with the tracked specification.
