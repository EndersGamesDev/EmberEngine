# Sealed Control Surface · `rec:controlsurface:sealed-control-surface`

**Status:** Decided

## Context · `sec:controlsurface:context`

The repository command contract establishes audit and append as controlled
readings of the declared lists and makes the response file the durable copy of
the stdout receipt (`rec:commandcontract:command-contract`). The package must
provide those semantics without giving either control argument authority to
inspect an arbitrary host path.

The former request option named a file which the process opened directly. The
former response guard canonicalized both the selected response and the declared
configuration directory before comparing them. A caller could therefore make
any readable host file the request, while the existence, permissions, and link
resolution of any response path could affect the guard's answer.

## Decision · `sec:controlsurface:decision`

**Decision (Request bytes arrive through the caller's stream)** · `dec:controlsurface:request-stream`

Standard input is the only command-line request transport for audit and append.
The command reads the stream whole after configuration preflight and passes its
bytes to the control operation. The public maintenance functions likewise take
a byte slice rather than a request path, so a library caller supplies data and
never delegates a filesystem read.

Request schema, validation, operation matching, authority, batching, and digest
semantics do not change. The response keeps its ``input`` member for schema
continuity and identifies its source as ``stdin``. The removed ``--input``
option has no path-based replacement.

**Decision (Receipt containment is lexical and sanctioned)** · `dec:controlsurface:lexical-receipt`

A response is accepted only when lexical normalization makes it a direct child
of the repository root. Normalization consumes path components in memory,
discarding current-directory components and resolving parent components without
querying the filesystem. The declared configuration directory, every direct
member of it, and every other nested or outside destination are refused before
the response writer runs.

The old canonicalization probe protected an existing response alias when its
resolved parent was the declared configuration directory. On a resolution
failure it fell back to the path as written, so the same spelling could receive
a different answer according to filesystem state. The replacement preserves
and tightens the write protection: every accepted response has the normalized
repository root itself as its parent, while the protected directory and its
members are rejected explicitly. A caller-selected parent path therefore
cannot alias the declared directory, and neither an absent response nor an
unreadable outside path changes the answer.

## Consequences · `sec:controlsurface:consequences`

Callers pipe the request and place the durable receipt at the corpus root:

```console
linter burn --audit --root . --output response.json < request.json
```

Append uses the same transport. Callers that formerly supplied ``--input`` must
migrate, and callers that placed receipts outside the root must move them after
the command completes if they need a different archival location. Response and
stdout equality, list mutation order, and control verdicts otherwise remain
unchanged.
