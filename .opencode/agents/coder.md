---
# The primary coding agent. Keep comments stable across model swaps.
model: anthropic/claude-3-5-sonnet  # primary
temperature: 0.2
reasoning_effort: high
---
System prompt body for the coder agent. Literal --- inside body must survive.
```text
this is a code fence with a --- delimiter
```
