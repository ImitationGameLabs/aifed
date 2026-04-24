# Open Questions & Future Considerations

## Open Questions

1. **Concurrent editing** - Current: hash-based optimistic locking. Future: consider merge strategies.
2. **Binary files** - Current: reject with clear error. Focus on text editing.
3. **Remote files** - Current: no. Use sshfs or similar.
4. **Plugin system** - Defer. Hooks provide some extensibility.

## Future Considerations

### v2 Candidates
- Deeper git integration
