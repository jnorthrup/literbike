# Track: Menu API/Models Listing 20260317

## Objective
Update the Literbike macOS icon menu to follow a new hierarchy for model listings.
The user wants: `xxxx/models/... -> LIST OF MODELS/ API`

## Current Implementation
`main.swift` has a `PROVIDERS` menu:
- `PROVIDERS` (top-level)
  - `PROVIDER` (e.g., `OPENAI`)
    - `MODEL_ID` (e.g., `gpt-4o`)

## Desired Implementation
New hierarchy:
- `API` (top-level)
  - `LIST OF MODELS` (sub-menu)
    - `PROVIDER` (e.g., `OPENAI`)
      - `models` (sub-menu)
        - `MODEL_ID` (e.g., `gpt-4o`)

## Plan
1. Research: Confirm `main.swift` is the only file requiring changes for the macOS menu.
2. Strategy: Update `updateMenu()` in `main.swift` to use the new hierarchy.
3. Execution:
   - Modify `main.swift` to rename `PROVIDERS` to `API`.
   - Add `LIST OF MODELS` sub-menu.
   - Add `models` sub-menu under each provider.
4. Validation:
   - Compile `main.swift` (if possible, though I don't have a macOS compiler here).
   - Verify the Swift code logic.
   - Since I cannot run the macOS app, I will ensure the code is syntactically correct and matches the patterns in the file.

## Verification
- Code review of `main.swift`.
- Check if `dynamic_models` parsing handles the new hierarchy correctly.
