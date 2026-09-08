# Folder README policy

Every directory that contains a tracked file also carries a tracked `README.md` so a worker or reviewer can understand a cold checkout without first loading chat history, tools or unrelated documents.

Each folder README stays under one screen and answers three questions specifically: what the folder holds, what code or workflow depends on it, and where the rules governing changes to it live.

Generated and vendored directories use a one-line README that identifies the directory as generated or vendored, names its source or update path, and warns against hand editing when appropriate.

The repository root README introduces the project and links to each top-level folder README; deeper READMEs describe only their own directory and point upward or to focused documentation instead of repeating repository-wide guidance.

Run `bash deploy/tests/test-readmes.sh` to list every tracked directory whose tracked `README.md` is missing. The check deliberately has no exception list: a tracked directory is either documented or reported.
