# Agent Instructions

## Interaction Modes

The user may explicitly specify one of the following modes as a style or working requirement. If no mode is specified, infer the appropriate mode from the request.

### Inquiry Mode / Discussion Mode

- Access files in read-only mode.
- Do not modify files.
- Do not execute any operation that changes data.
- If the user asks for a change while this mode is active, show the key solution code or patch conceptually instead of applying it to files.
- This mode remains active until the user explicitly asks to modify files.

### Edit Mode

- Access files in read-write mode.
- Carefully modify files and perform data-changing operations when they are necessary for the task.
- When the user asks for a change or raises a problem that has an actionable fix, proceed with the appropriate code changes.
- This mode remains active until the user explicitly asks to switch back to Inquiry Mode.

### Deep Dive Mode

- The user cares about underlying implementation details.
- When relevant, explain library internals, source-level behavior, or computer architecture and systems-level principles.
- If the answer introduces a technical term that has not appeared earlier in the conversation, define it and provide the necessary background and practical relevance.

## Tool And Command Restrictions

- Do not run functionality that an IDE can easily execute, including formatter, test, or check commands, unless the user explicitly asks for it.
- Treat requests to run formatter, tests, or checks as requiring explicit user intent in the current task.

## Dependency Policy

- Prefer dependencies that are already declared in dependency definition files, such as `Cargo.toml`.
- If no existing dependency can reasonably solve the problem, propose a new third-party library.
- Obtain the user's approval before adding or using a new third-party dependency.
