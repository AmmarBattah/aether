"""Held-out oracle for the scheduler case. Not copied into solver
workspaces; grade() imports the solver's `scheduler` package from their
workspace and runs these tests against it. Independent of whatever tests
the solver writes. No real time is used anywhere; only the virtual Clock."""
import heapq
import random
import unittest

from scheduler import Clock, Scheduler, SchedulerError, Task


class SchedulerHiddenTests(unittest.TestCase):
    def setUp(self):
        self.clock = Clock()
        self.scheduler = Scheduler(self.clock)
        self.calls = []

    def _record(self, name):
        def action():
            self.calls.append(name)
        return action

    # -- basic ordering / validation, also covered lightly by visible tests --

    def test_linear_chain_runs_in_dependency_order(self):
        self.scheduler.add_task(Task("a", action=self._record("a")))
        self.scheduler.add_task(Task("b", deps=["a"], action=self._record("b")))
        self.scheduler.add_task(Task("c", deps=["b"], action=self._record("c")))
        report = self.scheduler.run()
        self.assertEqual(self.calls, ["a", "b", "c"])
        self.assertEqual(report.order, ["a", "b", "c"])

    def test_unknown_dependency_raises_before_running_anything(self):
        self.scheduler.add_task(Task("a", deps=["ghost"], action=self._record("a")))
        with self.assertRaises(SchedulerError):
            self.scheduler.run()
        self.assertEqual(self.calls, [])

    def test_ready_tasks_run_in_ascending_id_order_not_push_order(self):
        for tid in ("z", "a", "m"):
            self.scheduler.add_task(Task(tid, action=self._record(tid)))
        report = self.scheduler.run()
        self.assertEqual(self.calls, ["a", "m", "z"])
        self.assertEqual(report.order, ["a", "m", "z"])

    def test_duplicate_task_id_raises_on_add_task(self):
        self.scheduler.add_task(Task("a", action=self._record("a")))
        with self.assertRaises(SchedulerError):
            self.scheduler.add_task(Task("a", action=self._record("a")))

    def test_task_without_an_action_raises_at_construction(self):
        with self.assertRaises(SchedulerError):
            Task("a", action=None)

    def test_run_report_results_cover_every_added_task(self):
        self.scheduler.add_task(Task("a", action=self._record("a")))
        self.scheduler.add_task(Task("b", deps=["a"], action=self._record("b")))
        report = self.scheduler.run()
        self.assertEqual(set(report.results), {"a", "b"})

    # -- cycle detection --

    def test_two_node_cycle_with_no_external_root_is_detected(self):
        self.scheduler.add_task(Task("a", deps=["b"], action=self._record("a")))
        self.scheduler.add_task(Task("b", deps=["a"], action=self._record("b")))
        with self.assertRaises(SchedulerError):
            self.scheduler.run()

    def test_diamond_shaped_dependencies_do_not_falsely_report_a_cycle(self):
        self.scheduler.add_task(Task("a", action=self._record("a")))
        self.scheduler.add_task(Task("b", deps=["a"], action=self._record("b")))
        self.scheduler.add_task(Task("c", deps=["a"], action=self._record("c")))
        self.scheduler.add_task(Task("d", deps=["b", "c"], action=self._record("d")))
        report = self.scheduler.run()
        self.assertEqual(report.results["d"].status, "success")
        self.assertEqual(set(self.calls), {"a", "b", "c", "d"})

    def test_self_loop_dependency_is_detected_as_a_cycle(self):
        self.scheduler.add_task(Task("a", deps=["a"], action=self._record("a")))
        with self.assertRaises(SchedulerError):
            self.scheduler.run()
        self.assertEqual(self.calls, [])

    def test_cycle_in_one_component_is_detected_even_when_another_has_a_root(self):
        self.scheduler.add_task(Task("root", action=self._record("root")))
        self.scheduler.add_task(Task("child", deps=["root"], action=self._record("child")))
        self.scheduler.add_task(Task("p", deps=["q"], action=self._record("p")))
        self.scheduler.add_task(Task("q", deps=["p"], action=self._record("q")))
        with self.assertRaises(SchedulerError):
            self.scheduler.run()
        self.assertEqual(self.calls, [])

    # -- skip propagation, including fan-in --

    def test_failed_dependency_skips_transitively_through_a_skipped_task(self):
        def boom():
            raise RuntimeError("boom")

        self.scheduler.add_task(Task("a", action=boom))
        self.scheduler.add_task(Task("b", deps=["a"], action=self._record("b")))
        self.scheduler.add_task(Task("c", deps=["b"], action=self._record("c")))
        report = self.scheduler.run()
        self.assertEqual(report.results["b"].status, "skipped")
        self.assertEqual(report.results["c"].status, "skipped")
        self.assertEqual(self.calls, [])

    def test_fan_in_task_is_skipped_if_any_of_several_dependencies_fails(self):
        def boom():
            raise RuntimeError("boom")

        self.scheduler.add_task(Task("a", action=self._record("a")))
        self.scheduler.add_task(Task("b", action=boom))
        self.scheduler.add_task(Task("c", deps=["a", "b"], action=self._record("c")))
        report = self.scheduler.run()
        self.assertEqual(report.results["c"].status, "skipped")
        self.assertNotIn("c", self.calls)

    def test_fan_in_task_runs_once_all_of_several_dependencies_succeed(self):
        self.scheduler.add_task(Task("a", action=self._record("a")))
        self.scheduler.add_task(Task("b", action=self._record("b")))
        self.scheduler.add_task(Task("c", deps=["a", "b"], action=self._record("c")))
        report = self.scheduler.run()
        self.assertEqual(report.results["c"].status, "success")
        self.assertIn("c", self.calls)

    # -- retries, backoff, deadlines --

    def test_retry_advances_the_shared_clock_by_backoff_between_attempts(self):
        attempts = {"n": 0}

        def flaky():
            attempts["n"] += 1
            if attempts["n"] < 2:
                raise RuntimeError("not yet")

        self.scheduler.add_task(Task("a", action=flaky, max_attempts=3, retry_backoff=2.5))
        report = self.scheduler.run()
        self.assertEqual(report.results["a"].status, "success")
        self.assertEqual(report.results["a"].attempts, 2)
        self.assertEqual(self.clock.now(), 2.5)

    def test_deadline_exceeded_mid_retry_stops_further_attempts(self):
        def fails_and_advances_clock_past_deadline():
            self.clock.advance(10)
            raise RuntimeError("slow")

        self.scheduler.add_task(Task(
            "a", action=fails_and_advances_clock_past_deadline,
            max_attempts=3, retry_backoff=0.0, deadline=5,
        ))
        report = self.scheduler.run()
        self.assertEqual(report.results["a"].status, "skipped")
        self.assertEqual(report.results["a"].error, "deadline exceeded")

    def test_deadline_already_passed_skips_without_calling_action(self):
        self.scheduler.add_task(Task("a", action=self._record("a"), deadline=-1))
        report = self.scheduler.run()
        self.assertEqual(report.results["a"].status, "skipped")
        self.assertEqual(self.calls, [])

    def test_deadline_equal_to_current_clock_time_is_not_exceeded(self):
        self.scheduler.add_task(Task("a", action=self._record("a"), deadline=0))
        report = self.scheduler.run()
        self.assertEqual(report.results["a"].status, "success")
        self.assertEqual(self.calls, ["a"])

    def test_failed_task_result_carries_the_last_attempts_error_message(self):
        attempts = {"n": 0}

        def flaky():
            attempts["n"] += 1
            raise RuntimeError(f"failure #{attempts['n']}")

        self.scheduler.add_task(Task("a", action=flaky, max_attempts=3, retry_backoff=0.0))
        report = self.scheduler.run()
        self.assertEqual(report.results["a"].status, "failed")
        self.assertEqual(report.results["a"].attempts, 3)
        self.assertEqual(report.results["a"].error, "failure #3")

    def test_clock_advance_accumulates_instead_of_overwriting(self):
        clock = Clock()
        clock.advance(3)
        clock.advance(2)
        self.assertEqual(clock.now(), 5)

    # -- input validation --

    def test_negative_retry_backoff_is_rejected_at_task_construction(self):
        with self.assertRaises(SchedulerError):
            Task("a", action=self._record("a"), retry_backoff=-1)

    def test_max_attempts_below_one_is_rejected_at_task_construction(self):
        with self.assertRaises(SchedulerError):
            Task("a", action=self._record("a"), max_attempts=0)

    # -- seeded/property-style deterministic sequence, independent oracle --

    def test_generated_dag_ordering_matches_an_independent_topological_oracle(self):
        for seed in range(20):
            labels, deps = _random_dag(seed)
            clock = Clock()
            scheduler = Scheduler(clock)
            for label in labels:
                scheduler.add_task(Task(label, deps=deps[label], action=lambda: None))
            report = scheduler.run()
            expected = _expected_topological_order(labels, deps)
            self.assertEqual(report.order, expected, f"seed={seed}")
            self.assertTrue(all(r.status == "success" for r in report.results.values()),
                             f"seed={seed}")


def _random_dag(seed, node_count=8, edge_prob=0.35):
    """A deterministic (seeded), acyclic-by-construction dependency graph:
    edges only go from a node to an earlier one in a fixed random topological
    permutation, so it is guaranteed acyclic regardless of which edges land."""
    rng = random.Random(seed)
    labels = [f"t{i}" for i in range(node_count)]
    permutation = labels[:]
    rng.shuffle(permutation)
    deps = {label: [] for label in labels}
    for i in range(node_count):
        for j in range(i):
            if rng.random() < edge_prob:
                deps[permutation[i]].append(permutation[j])
    return labels, deps


def _expected_topological_order(labels, deps):
    """Kahn's algorithm with a min-heap keyed by task id: an independent
    canonical oracle for the contract's 'ascending task_id among ready
    tasks' ordering rule, without reusing any scheduler package code."""
    indegree = {label: len(deps[label]) for label in labels}
    dependents = {label: [] for label in labels}
    for label, its_deps in deps.items():
        for dep in its_deps:
            dependents[dep].append(label)
    heap = sorted(label for label in labels if indegree[label] == 0)
    heapq.heapify(heap)
    order = []
    while heap:
        label = heapq.heappop(heap)
        order.append(label)
        for dependent in dependents[label]:
            indegree[dependent] -= 1
            if indegree[dependent] == 0:
                heapq.heappush(heap, dependent)
    assert len(order) == len(labels), "oracle bug: generated graph must be acyclic"
    return order


if __name__ == "__main__":
    unittest.main()
