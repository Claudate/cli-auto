# Multi-window prompts sample (serial-prompts/v0)

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
End with: CCO_DONE ok
```

### t2 · feature A

```
Write FEATURE_A.md describing the change.
End with: CCO_DONE ok
```

### t3 · integrate

```
Write INTEGRATION.md with three bullet points.
End with: CCO_DONE ok
```
