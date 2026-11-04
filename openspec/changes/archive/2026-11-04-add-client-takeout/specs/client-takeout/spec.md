# Spec Delta

## Purpose

Defines the data-export (takeout) session wrapper.

## ADDED Requirements

### Requirement: Takeout session

The client SHALL initialize a takeout session, wrap calls made inside the closure with
`invokeWithTakeout`, and finish the session when the closure returns.

#### Scenario: Session is finished on success
- **WHEN** the closure completes
- **THEN** `account.finishTakeoutSession` is called

#### Scenario: Session is finished on error
- **WHEN** the closure returns an error
- **THEN** the session is still finished before the error propagates
