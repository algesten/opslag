# Unreleased

# 0.2.3

  * Fix duplicate query targets in `Server::query()` causing request flood
  * Deduplicate answers in server response to duplicate queries
  * Make `Writer` overflow-safe to prevent panic on buffer exhaustion

# 0.2.2

  * Discovery-only mode via `Server::query()`

# 0.2.1

  * Fix error with defmt flag

# 0.2.0

  * Multihome support
  * defmt as feature flag

# 0.1.0

  * First release
