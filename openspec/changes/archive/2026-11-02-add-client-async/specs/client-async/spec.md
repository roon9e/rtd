# Spec Delta

## Purpose

Defines the asynchronous façade over the synchronous MTProto client core for tokio callers.

## ADDED Requirements

### Requirement: Asynchronous client façade

The client SHALL expose an asynchronous façade whose `initialize` and `invoke` do not block a
tokio runtime worker, by serving the synchronous client core from a dedicated actor thread
behind a command channel.

#### Scenario: An async call completes
- **WHEN** an async caller invokes a method
- **THEN** the call is served by the actor thread and the decoded result is returned without blocking the runtime

#### Scenario: Startup failure is reported
- **WHEN** the transport cannot be dialed or the key exchange fails during spawn
- **THEN** the asynchronous constructor returns an error rather than deferring it to the first call

#### Scenario: Drop shuts the actor down
- **WHEN** the asynchronous client is dropped
- **THEN** the actor thread ends and its socket is released
