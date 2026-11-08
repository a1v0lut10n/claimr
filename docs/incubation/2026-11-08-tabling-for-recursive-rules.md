# Tabling, or termination for recursive rules

Captured 2026-11-08. Prompted by rules over a knowledge graph. Transitive predicates are
common there:

- part-of chains (step → tree → project);
- derived-from lineage;
- broader-than in a concept hierarchy.

Under SLD resolution, the natural left-recursive definition

```
ancestor(X, Z) :- ancestor(X, Y), parent(Y, Z).
ancestor(X, Y) :- parent(X, Y).
```

loops. A right-recursive version terminates only on acyclic data, and real graphs can
hold cycles through data errors.

The idea is tabling (SLG resolution, as in XSB and SWI-Prolog's `:- table`): declared
predicates memoise their answers, so they terminate on finite data and cycles, and avoid
recomputing. In claimr, tabling would have to account for constraints in answers. Answer
subsumption over constraint stores is a research-grade topic, so a first version might
table only predicates whose answers are ground, and raise an error otherwise.

A smaller first step, if tabling is too much: a documented depth or step limit that
reports non-termination as an error, never as a hang.

It interacts with `2026-11-08-negation-as-failure.md` (stratification) and
`2026-11-08-aggregation-and-all-solutions.md`.
