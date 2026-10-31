//! The draw: assign every participant someone to give to.
//!
//! Constraints: nobody draws themselves, and excluded pairs never draw each other (in either
//! direction). We first look for a single cycle through everyone — the classic amigo oculto
//! shape, which also rules out two people drawing each other — using randomized depth-first
//! search with a step budget. If no cycle is found (heavy exclusions can make one impossible),
//! we fall back to any valid derangement via randomized bipartite matching, and only report
//! infeasibility when that fails too.

use std::collections::HashSet;

use rand::seq::SliceRandom;

use crate::ids::ParticipantId;
use crate::models::Exclusion;

pub const MIN_PARTICIPANTS: usize = 3;
const CYCLE_SEARCH_BUDGET: usize = 200_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DrawError {
    #[error("são necessários pelo menos {MIN_PARTICIPANTS} participantes")]
    TooFew,
    #[error("com essas restrições não existe sorteio possível")]
    Infeasible,
}

/// Returns `(giver, receiver)` pairs covering every participant exactly once on each side.
pub fn draw(
    participants: &[ParticipantId],
    exclusions: &[Exclusion],
) -> Result<Vec<(ParticipantId, ParticipantId)>, DrawError> {
    let n = participants.len();
    if n < MIN_PARTICIPANTS {
        return Err(DrawError::TooFew);
    }
    let excluded: HashSet<Exclusion> = exclusions.iter().copied().collect();
    let allowed = |i: usize, j: usize| {
        i != j && !excluded.contains(&Exclusion::new(participants[i], participants[j]))
    };
    let adjacency: Vec<Vec<usize>> = (0..n)
        .map(|i| (0..n).filter(|&j| allowed(i, j)).collect())
        .collect();

    let mut rng = rand::rng();
    let receivers = find_cycle(&adjacency, &mut rng)
        .or_else(|| find_matching(&adjacency, &mut rng))
        .ok_or(DrawError::Infeasible)?;

    Ok((0..n)
        .map(|i| (participants[i], participants[receivers[i]]))
        .collect())
}

/// Hamiltonian cycle by randomized DFS. Returns `receiver[i]` = next node after `i`.
fn find_cycle(adj: &[Vec<usize>], rng: &mut impl rand::Rng) -> Option<Vec<usize>> {
    let n = adj.len();
    let start = rng.random_range(0..n);
    let mut path = vec![start];
    let mut visited = vec![false; n];
    visited[start] = true;
    let mut budget = CYCLE_SEARCH_BUDGET;

    fn step(
        adj: &[Vec<usize>],
        start: usize,
        path: &mut Vec<usize>,
        visited: &mut [bool],
        budget: &mut usize,
        rng: &mut impl rand::Rng,
    ) -> bool {
        if *budget == 0 {
            return false;
        }
        *budget -= 1;
        let n = adj.len();
        let here = *path.last().expect("path is never empty");
        if path.len() == n {
            return adj[here].contains(&start);
        }
        // Prune: every unvisited node still needs an unvisited (or start) successor.
        let mut candidates: Vec<usize> =
            adj[here].iter().copied().filter(|&j| !visited[j]).collect();
        if candidates.is_empty() {
            return false;
        }
        candidates.shuffle(rng);
        for next in candidates {
            visited[next] = true;
            path.push(next);
            if step(adj, start, path, visited, budget, rng) {
                return true;
            }
            path.pop();
            visited[next] = false;
            if *budget == 0 {
                return false;
            }
        }
        false
    }

    if !step(adj, start, &mut path, &mut visited, &mut budget, rng) {
        return None;
    }
    let mut receivers = vec![0; n];
    for (k, &node) in path.iter().enumerate() {
        receivers[node] = path[(k + 1) % n];
    }
    Some(receivers)
}

/// Perfect matching givers → receivers (Kuhn's algorithm with randomized order). Any perfect
/// matching under `allowed` is a valid derangement, possibly with short cycles.
fn find_matching(adj: &[Vec<usize>], rng: &mut impl rand::Rng) -> Option<Vec<usize>> {
    let n = adj.len();
    let mut match_of_receiver: Vec<Option<usize>> = vec![None; n];

    fn augment(
        giver: usize,
        adj: &[Vec<usize>],
        seen: &mut [bool],
        match_of_receiver: &mut [Option<usize>],
        order: &[usize],
    ) -> bool {
        for &receiver in order.iter().filter(|r| adj[giver].contains(r)) {
            if seen[receiver] {
                continue;
            }
            seen[receiver] = true;
            let free = match match_of_receiver[receiver] {
                None => true,
                Some(other) => augment(other, adj, seen, match_of_receiver, order),
            };
            if free {
                match_of_receiver[receiver] = Some(giver);
                return true;
            }
        }
        false
    }

    let mut givers: Vec<usize> = (0..n).collect();
    givers.shuffle(rng);
    let mut order: Vec<usize> = (0..n).collect();
    for giver in givers {
        order.shuffle(rng);
        let mut seen = vec![false; n];
        if !augment(giver, adj, &mut seen, &mut match_of_receiver, &order) {
            return None;
        }
    }
    let mut receivers = vec![0; n];
    for (receiver, giver) in match_of_receiver.into_iter().enumerate() {
        receivers[giver?] = receiver;
    }
    Some(receivers)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(n: usize) -> Vec<ParticipantId> {
        (0..n).map(|_| ParticipantId::new()).collect()
    }

    fn check(ids: &[ParticipantId], ex: &[Exclusion], pairs: &[(ParticipantId, ParticipantId)]) {
        assert_eq!(pairs.len(), ids.len());
        let givers: HashSet<_> = pairs.iter().map(|p| p.0).collect();
        let receivers: HashSet<_> = pairs.iter().map(|p| p.1).collect();
        assert_eq!(givers.len(), ids.len(), "every participant gives once");
        assert_eq!(
            receivers.len(),
            ids.len(),
            "every participant receives once"
        );
        let excluded: HashSet<_> = ex.iter().copied().collect();
        for (g, r) in pairs {
            assert_ne!(g, r, "nobody draws themselves");
            assert!(
                !excluded.contains(&Exclusion::new(*g, *r)),
                "exclusion respected"
            );
        }
    }

    fn is_single_cycle(pairs: &[(ParticipantId, ParticipantId)]) -> bool {
        let next: std::collections::HashMap<_, _> = pairs.iter().copied().collect();
        let start = pairs[0].0;
        let mut cur = next[&start];
        let mut len = 1;
        while cur != start {
            cur = next[&cur];
            len += 1;
        }
        len == pairs.len()
    }

    #[test]
    fn rejects_too_few() {
        assert_eq!(draw(&ids(2), &[]), Err(DrawError::TooFew));
    }

    #[test]
    fn plain_draw_is_a_single_cycle() {
        for n in [3, 4, 7, 25, 100] {
            let p = ids(n);
            let pairs = draw(&p, &[]).unwrap();
            check(&p, &[], &pairs);
            assert!(is_single_cycle(&pairs));
        }
    }

    #[test]
    fn respects_couples() {
        let p = ids(8);
        let ex = vec![
            Exclusion::new(p[0], p[1]),
            Exclusion::new(p[2], p[3]),
            Exclusion::new(p[4], p[5]),
            Exclusion::new(p[6], p[7]),
        ];
        for _ in 0..50 {
            let pairs = draw(&p, &ex).unwrap();
            check(&p, &ex, &pairs);
            assert!(is_single_cycle(&pairs));
        }
    }

    #[test]
    fn three_people_one_couple_is_infeasible() {
        let p = ids(3);
        let ex = vec![Exclusion::new(p[0], p[1])];
        // The only derangements of three people are the two 3-cycles, and each uses
        // either 0→1 or 1→0; with a symmetric exclusion there is no way out.
        assert_eq!(draw(&p, &ex), Err(DrawError::Infeasible));
    }

    #[test]
    fn four_people_one_couple_is_feasible() {
        let p = ids(4);
        let ex = vec![Exclusion::new(p[0], p[1])];
        for _ in 0..20 {
            let pairs = draw(&p, &ex).unwrap();
            check(&p, &ex, &pairs);
            assert!(is_single_cycle(&pairs));
        }
    }

    #[test]
    fn detects_infeasible() {
        let p = ids(3);
        // 0 may not draw 1 or 2 → 0 has nobody left to give to.
        let ex = vec![Exclusion::new(p[0], p[1]), Exclusion::new(p[0], p[2])];
        assert_eq!(draw(&p, &ex), Err(DrawError::Infeasible));
    }

    #[test]
    fn falls_back_to_derangement_when_no_cycle_exists() {
        let p = ids(4);
        // 0 may only exchange with 1, which forces the 2-cycle 0↔1; no cycle through all four
        // exists, but 2↔3 completes a valid derangement.
        let ex = vec![Exclusion::new(p[0], p[2]), Exclusion::new(p[0], p[3])];
        let pairs = draw(&p, &ex).unwrap();
        check(&p, &ex, &pairs);
        assert!(!is_single_cycle(&pairs));
    }

    #[test]
    fn large_group_with_many_couples() {
        let p = ids(60);
        let ex: Vec<_> = (0..30)
            .map(|k| Exclusion::new(p[2 * k], p[2 * k + 1]))
            .collect();
        let pairs = draw(&p, &ex).unwrap();
        check(&p, &ex, &pairs);
    }
}
