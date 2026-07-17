# Multi-window prompts sample (serial-prompts/v0 golden)

This fixture mimics legacy multi-window plan markdown.

## Graph

| id | title | group | depends_on | mode |
|----|-------|-------|------------|------|
| t1 | setup docs | G1 | | print |
| t2 | feature A | G1 | | print |
| t3 | integrate | G2 | t1,t2 | print |

## Tasks

### t1 · setup docs

```
Create docs/overview.md with a short project summary.
Do not modify source code.
End with: CCO_DONE ok
```

### t2 · feature A

```
Add a tiny helper module or note file named FEATURE_A.md.
Keep the change isolated.
End with: CCO_DONE ok
```

### t3 · integrate

```
Review t1 and t2 outputs. Write INTEGRATION.md with three bullet points.
End with: CCO_DONE ok
```

## Notes

Ignore this section — not a task.
