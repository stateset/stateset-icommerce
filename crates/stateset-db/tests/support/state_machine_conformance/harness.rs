//! Drives one catalogued machine through the real repositories and compares
//! every outcome with the catalog.
//!
//! A [`Conformance`] knows how to create a record and which repository
//! operations move it, each tagged with the status it moves the record into.
//! [`Conformance::check`] then, for every state the operations can reach and
//! for every operation, takes a fresh record to that state along catalogued
//! moves, runs the operation and asserts:
//!
//! * a move the catalog lists is accepted and stores the target status;
//! * a move the catalog does not list is refused (an error) and leaves the
//!   status unchanged;
//! * a same-status operation is accepted exactly when the catalog marks the
//!   state reentrant;
//! * every catalogued edge was made by at least one operation, and every
//!   state saw at least one refusal (unless every operation is allowed there).
//!
//! An operation that returns `Ok` without moving the record is a silent no-op;
//! it fails the check unless the operation was declared `silent`, because a
//! caller cannot tell it from success.
//!
//! A move whose outcome depends on the record's history (resolving a dispute
//! returns a bill to the status it was disputed from) is proven with a
//! [`scenario`](Conformance::scenario): an explicit route into the state,
//! then the operation, which must land on the catalogued target.
//!
//! A state that no operation produces (a status only legacy data or another
//! writer could hold) can be `seeded`: the check writes it straight into the
//! table with [`SetStatus`] and then drives every operation from it, so the
//! moves the catalog lists out of it are proven too. A move into a seeded
//! state, like every other catalogued move, must be made by an operation or a
//! scenario (a dispute resolved back to the seeded status it came from).

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt::Debug;

use stateset_core::state_machine::{StateMachineDescriptor, describe};
use stateset_core::{CommerceError, Result};

type Create<'a, Id> = Box<dyn Fn() -> Id + 'a>;
type Status<'a, Id> = Box<dyn Fn(Id) -> String + 'a>;
type Apply<'a, Id> = Box<dyn Fn(Id) -> Result<()> + 'a>;
type Seed<'a, Id> = Box<dyn Fn(Id) + 'a>;

/// Writes `status` into `table` for the row whose id is `id`, bypassing the
/// repositories (`(table, id, status)`).
pub(crate) type SetStatus<'a> = &'a dyn Fn(&str, &str, &str);

struct Operation<'a, Id> {
    target: &'static str,
    label: &'static str,
    apply: Apply<'a, Id>,
    /// A refused move returns `Ok` and leaves the record as it was.
    silent: bool,
    /// Refuses a same-status move even where the state is reentrant through
    /// another operation.
    refuses_same_status: bool,
    /// Makes its move only from these states and is refused from every other
    /// state, even where another operation makes the same move.
    only_from: Option<&'static [&'static str]>,
    /// Run only from these states (its precondition cannot be set up from
    /// the others); skipped elsewhere.
    run_from: Option<&'static [&'static str]>,
}

/// How a check takes a fresh record to a state.
#[derive(Clone, Default)]
struct Route {
    /// Written straight into the record first, if the route starts at a
    /// seeded state rather than the initial one.
    seed: Option<String>,
    /// Catalogued moves made after that, in order.
    hops: Vec<String>,
}

/// One machine's repository operations, ready to be checked.
pub(crate) struct Conformance<'a, Id> {
    machine: StateMachineDescriptor,
    create: Create<'a, Id>,
    status: Status<'a, Id>,
    operations: Vec<Operation<'a, Id>>,
    seeds: BTreeMap<&'static str, Seed<'a, Id>>,
    unreachable: BTreeSet<&'static str>,
    scenarios: Vec<Scenario<'a, Id>>,
}

struct Scenario<'a, Id> {
    route: &'static [&'static str],
    label: &'static str,
    target: &'static str,
    apply: Apply<'a, Id>,
}

/// What a check exercised, for the test log.
#[derive(Debug)]
pub(crate) struct Report {
    pub(crate) machine: String,
    pub(crate) states: usize,
    pub(crate) edges: usize,
    pub(crate) accepted: usize,
    pub(crate) refused: usize,
    pub(crate) noops: usize,
}

impl std::fmt::Display for Report {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}: {} states, {} edges; {} moves accepted, {} refused, {} same-status no-ops",
            self.machine, self.states, self.edges, self.accepted, self.refused, self.noops
        )
    }
}

impl<'a, Id: Copy + Debug> Conformance<'a, Id> {
    /// `create` makes a record in the machine's initial state; `status` reads
    /// its stored status as the wire name.
    pub(crate) fn new(
        machine: &str,
        create: impl Fn() -> Id + 'a,
        status: impl Fn(Id) -> String + 'a,
    ) -> Self {
        let machine = describe(machine).unwrap_or_else(|| panic!("{machine} is not catalogued"));
        Self {
            machine,
            create: Box::new(create),
            status: Box::new(status),
            operations: Vec::new(),
            seeds: BTreeMap::new(),
            unreachable: BTreeSet::new(),
            scenarios: Vec::new(),
        }
    }

    /// A history-dependent move: take a fresh record along `route` (state
    /// names; the first may be a seeded state, every other hop is made by the
    /// first operation that makes it), run `apply`, and require it to land on
    /// `target` through a catalogued move.
    pub(crate) fn scenario(
        mut self,
        route: &'static [&'static str],
        label: &'static str,
        target: &'static str,
        apply: impl Fn(Id) -> Result<()> + 'a,
    ) -> Self {
        self.scenarios.push(Scenario { route, label, target, apply: Box::new(apply) });
        self
    }

    /// A state no operation produces, written straight into the record by
    /// `seed` so the moves out of it can be driven.
    pub(crate) fn seeded(mut self, state: &'static str, seed: impl Fn(Id) + 'a) -> Self {
        self.seeds.insert(state, Box::new(seed));
        self
    }

    /// A repository operation that moves a record into `target`.
    pub(crate) fn op(
        mut self,
        target: &'static str,
        label: &'static str,
        apply: impl Fn(Id) -> Result<()> + 'a,
    ) -> Self {
        self.operations.push(Operation {
            target,
            label,
            apply: Box::new(apply),
            silent: false,
            refuses_same_status: false,
            only_from: None,
            run_from: None,
        });
        self
    }

    /// The last operation added moves a record only out of `states` (a
    /// precondition beyond the status, e.g. spending a balance that only an
    /// active credit has); from any other state it is refused even where the
    /// catalog lists the move for another operation.
    pub(crate) fn only_from(mut self, states: &'static [&'static str]) -> Self {
        self.operations.last_mut().expect("an operation").only_from = Some(states);
        self
    }

    /// The last operation added is run only from `states`: its precondition
    /// (e.g. an application to reverse) exists only there, so it is skipped
    /// from every other state rather than expected to be refused.
    pub(crate) fn run_from(mut self, states: &'static [&'static str]) -> Self {
        self.operations.last_mut().expect("an operation").run_from = Some(states);
        self
    }

    /// The last operation added refuses a same-status move even where the
    /// state is reentrant through another operation (e.g. `apply` on a
    /// depleted store credit, which `adjust(0)` accepts).
    pub(crate) fn refuses_same_status(mut self) -> Self {
        self.operations.last_mut().expect("an operation").refuses_same_status = true;
        self
    }

    /// Like [`op`](Self::op), for an operation that answers a refused move
    /// with `Ok` and leaves the record unchanged (a conditional `UPDATE`).
    pub(crate) fn silent_op(
        mut self,
        target: &'static str,
        label: &'static str,
        apply: impl Fn(Id) -> Result<()> + 'a,
    ) -> Self {
        self.operations.push(Operation {
            target,
            label,
            apply: Box::new(apply),
            silent: true,
            refuses_same_status: false,
            only_from: None,
            run_from: None,
        });
        self
    }

    /// A state no repository operation moves a record into. Its outgoing
    /// moves cannot be driven; the catalog must list no move into it.
    pub(crate) fn unreachable(mut self, state: &'static str) -> Self {
        self.unreachable.insert(state);
        self
    }

    fn state(&self, name: &str) -> &stateset_core::StateDescriptor {
        self.machine
            .state(name)
            .unwrap_or_else(|| panic!("{}: {name} is not a state", self.machine.name))
    }

    /// Shortest route to every state: from the initial state (or a seeded
    /// state) along catalogued moves that some operation makes.
    fn routes(&self) -> BTreeMap<String, Route> {
        let initial = self.machine.initial.clone();
        let mut routes = BTreeMap::from([(initial.clone(), Route::default())]);
        let mut queue = VecDeque::from([initial]);
        for seeded in self.seeds.keys() {
            let route = Route { seed: Some((*seeded).to_owned()), hops: Vec::new() };
            routes.insert((*seeded).to_owned(), route);
            queue.push_back((*seeded).to_owned());
        }
        while let Some(at) = queue.pop_front() {
            let route = routes[&at].clone();
            for next in &self.state(&at).next {
                if routes.contains_key(next) || !self.operations.iter().any(|o| o.target == next) {
                    continue;
                }
                let mut extended = route.clone();
                extended.hops.push(next.clone());
                routes.insert(next.clone(), extended);
                queue.push_back(next.clone());
            }
        }
        routes
    }

    /// A fresh record taken along `route` with the first operation that makes
    /// each hop.
    fn reach(&self, route: &Route) -> Id {
        let id = (self.create)();
        assert_eq!(
            (self.status)(id),
            self.machine.initial,
            "{}: a new record starts in the catalog's initial state",
            self.machine.name
        );
        if let Some(seeded) = &route.seed {
            (self.seeds[seeded.as_str()])(id);
            assert_eq!((self.status)(id), *seeded, "{}: seeded {seeded}", self.machine.name);
        }
        for hop in &route.hops {
            let from = (self.status)(id);
            let accepted = self
                .operations
                .iter()
                .filter(|o| o.target == hop.as_str())
                .find_map(|o| (o.apply)(id).ok().filter(|()| (self.status)(id) == *hop));
            assert!(
                accepted.is_some(),
                "{}: no operation made the catalogued move {from} -> {hop}",
                self.machine.name
            );
        }
        id
    }

    /// Run every operation from every reachable state and compare with the
    /// catalog. Panics on the first disagreement.
    pub(crate) fn check(self) -> Report {
        let name = self.machine.name.clone();
        for op in &self.operations {
            self.state(op.target);
        }
        let routes = self.routes();
        let unreachable: BTreeSet<&str> = self
            .machine
            .states
            .iter()
            .map(|s| s.name.as_str())
            .filter(|s| !routes.contains_key(*s))
            .collect();
        assert_eq!(
            unreachable,
            self.unreachable.iter().copied().collect::<BTreeSet<&str>>(),
            "{name}: states no operation reaches"
        );
        for state in &self.machine.states {
            for next in &state.next {
                assert!(
                    !self.unreachable.contains(next.as_str()),
                    "{name}: the catalog lists {} -> {next}, but no operation reaches {next}",
                    state.name
                );
            }
        }

        let mut made: BTreeSet<(String, String)> = BTreeSet::new();
        let (mut accepted, mut refused, mut noops) = (0, 0, 0);
        for state in &self.machine.states {
            let Some(route) = routes.get(&state.name) else { continue };
            let (mut refused_here, mut noops_here) = (0, 0);
            for op in &self.operations {
                if op.run_from.is_some_and(|from| !from.contains(&state.name.as_str())) {
                    continue;
                }
                let id = self.reach(route);
                let outcome: std::result::Result<(), CommerceError> = (op.apply)(id);
                let after = (self.status)(id);
                let context = format!(
                    "{name}: {} -> {} via {} (catalog: {:?})",
                    state.name, op.target, op.label, state.next
                );
                let applies = op.only_from.is_none_or(|from| from.contains(&state.name.as_str()));
                if op.target == state.name {
                    let expected = applies && state.reentrant && !op.refuses_same_status;
                    assert_eq!(
                        outcome.is_ok(),
                        expected,
                        "{context}: same-status outcome {outcome:?} vs reentrant={}",
                        state.reentrant
                    );
                    assert_eq!(after, state.name, "{context}: a same-status move kept the status");
                    if outcome.is_ok() {
                        noops += 1;
                        noops_here += 1;
                    } else {
                        refused += 1;
                        refused_here += 1;
                    }
                    continue;
                }
                if applies && state.next.iter().any(|n| n == op.target) {
                    assert!(outcome.is_ok(), "{context}: refused a catalogued move: {outcome:?}");
                    assert_eq!(after, op.target, "{context}: accepted but stored {after}");
                    made.insert((state.name.clone(), op.target.to_owned()));
                    accepted += 1;
                } else {
                    assert_eq!(after, state.name, "{context}: moved to {after} ({outcome:?})");
                    assert!(
                        outcome.is_err() || op.silent,
                        "{context}: answered Ok without moving the record"
                    );
                    refused += 1;
                    refused_here += 1;
                }
            }
            let all_allowed = self
                .operations
                .iter()
                .filter(|o| o.run_from.is_none_or(|from| from.contains(&state.name.as_str())))
                .all(|o| state.next.iter().any(|n| n == o.target) || o.target == state.name);
            assert!(
                refused_here > 0 || all_allowed,
                "{name}: no move out of {} was refused",
                state.name
            );
            assert!(
                !state.reentrant || noops_here > 0,
                "{name}: {} is marked reentrant but no operation repeated it",
                state.name
            );
        }
        for scenario in &self.scenarios {
            let (first, rest) = scenario.route.split_first().expect("a scenario route");
            let route = if self.seeds.contains_key(first) {
                Route {
                    seed: Some((*first).to_owned()),
                    hops: rest.iter().map(|s| (*s).to_owned()).collect(),
                }
            } else {
                Route { seed: None, hops: scenario.route.iter().map(|s| (*s).to_owned()).collect() }
            };
            let from = scenario.route.last().expect("a scenario route");
            let context = format!(
                "{name}: {} -> {} via {} (scenario {:?})",
                from, scenario.target, scenario.label, scenario.route
            );
            assert!(
                self.state(from).next.iter().any(|n| n == scenario.target),
                "{context}: not a catalogued move"
            );
            let id = self.reach(&route);
            let outcome = (scenario.apply)(id);
            assert!(outcome.is_ok(), "{context}: refused: {outcome:?}");
            assert_eq!((self.status)(id), scenario.target, "{context}");
            made.insert(((*from).to_owned(), scenario.target.to_owned()));
            accepted += 1;
        }
        let mut edges = 0;
        for state in &self.machine.states {
            for next in &state.next {
                edges += 1;
                assert!(
                    made.contains(&(state.name.clone(), next.clone())),
                    "{name}: no operation made the catalogued move {} -> {next}",
                    state.name
                );
            }
        }
        Report { machine: name, states: self.machine.states.len(), edges, accepted, refused, noops }
    }
}
