# Repository ruleset payloads

These files are complete request bodies for GitHub's repository-ruleset REST endpoints, so the reviewable policy and the externally applied rules can be compared byte for byte. Each body is accepted by `gh api --method POST repos/EndersGamesDev/EmberEngine/rulesets --input FILE`; the same body is accepted by the corresponding `PUT` endpoint when an existing ruleset is replaced.

`develop.json` records live ruleset `22736795`, `main-authorization.json` records live ruleset `22736890`, `main-integrity.json` records live ruleset `22795584`, `release-tags-authorization.json` records live ruleset `22795757`, `release-tags-integrity.json` records live ruleset `22795767`, `other-tags.json` records live ruleset `22795785`, and `branch-names.json` records live ruleset `22770318`. The payloads remain separate because integrity rules carry no bypass actor while authorization rules grant only the narrowly required creation or update path; the branch-name ruleset admits new names only under the documented integration, release, feature and lane patterns without changing existing refs.

`gh-pages-frozen.json` has no live ruleset id before integration; once the payload is integrated, it is applied and its id is recorded here. Its no-bypass `update` and `deletion` rules preserve `refs/heads/gh-pages` as the immutable publication record, while `non_fast_forward` is absent because `update` already refuses every push to the ref, whether fast-forward or not.

Branch commit signatures are not required for now. Release provenance remains signed because `release.yml` verifies annotated release tags against `deploy/keys/`; a later branch-signature policy can add `required_signatures` to both `develop.json` and `main-integrity.json` without changing the authorization payloads.

The JSON files contain no repository identifier or credential. Applying them is an external repository-administration operation, and drift between the active settings and these bodies is checked by comparing the API response with the tracked specification.
