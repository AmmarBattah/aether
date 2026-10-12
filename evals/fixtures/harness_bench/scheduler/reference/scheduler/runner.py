"""Dependency-aware task execution engine.

See ../../starter/CONTRACT.md. This is the reference implementation used
only to validate the grader; it is never copied into a solver workspace.
"""
from .errors import SchedulerError
from .graph import DependencyGraph
from .queue import ReadyQueue


class Task:
    def __init__(self, task_id, deps=(), action=None, max_attempts=1,
                 retry_backoff=0.0, deadline=None):
        if action is None:
            raise SchedulerError(f"task {task_id!r} requires an action")
        if max_attempts < 1:
            raise SchedulerError(f"task {task_id!r} needs max_attempts >= 1")
        if retry_backoff < 0:
            raise SchedulerError(f"task {task_id!r} needs retry_backoff >= 0")
        self.task_id = task_id
        self.deps = list(deps)
        self.action = action
        self.max_attempts = max_attempts
        self.retry_backoff = retry_backoff
        self.deadline = deadline


class TaskResult:
    def __init__(self, status, attempts=0, error=None):
        self.status = status
        self.attempts = attempts
        self.error = error

    def __repr__(self):
        return f"TaskResult(status={self.status!r}, attempts={self.attempts}, error={self.error!r})"


class RunReport:
    def __init__(self, order, results):
        self.order = order
        self.results = results


class Scheduler:
    def __init__(self, clock):
        self._clock = clock
        self._tasks = {}

    def add_task(self, task):
        if task.task_id in self._tasks:
            raise SchedulerError(f"duplicate task id: {task.task_id}")
        self._tasks[task.task_id] = task

    def run(self):
        self._validate()
        self._build_graph().check_acyclic()
        return self._execute()

    def _validate(self):
        for tid, task in self._tasks.items():
            for dep in task.deps:
                if dep not in self._tasks:
                    raise SchedulerError(f"unknown dependency: {dep!r} required by {tid!r}")

    def _build_graph(self):
        graph = DependencyGraph()
        for tid in self._tasks:
            graph.add_node(tid)
        for tid, task in self._tasks.items():
            for dep in task.deps:
                graph.add_edge(tid, dep)
        return graph

    def _execute(self):
        results = {}
        order = []
        remaining = {tid: set(t.deps) for tid, t in self._tasks.items()}
        ready = ReadyQueue()
        for tid, deps in remaining.items():
            if not deps:
                ready.push(tid)

        while not ready.is_empty():
            tid = ready.pop_next()
            task = self._tasks[tid]
            if any(results[d].status != "success" for d in task.deps):
                results[tid] = TaskResult("skipped", attempts=0,
                                           error="a dependency did not succeed")
            else:
                order.append(tid)
                results[tid] = self._run_task(task)
            self._release_dependents(tid, remaining, results, ready)

        return RunReport(order, results)

    def _run_task(self, task):
        attempts = 0
        last_error = None
        while attempts < task.max_attempts:
            if task.deadline is not None and self._clock.now() > task.deadline:
                return TaskResult("skipped", attempts=0, error="deadline exceeded")
            attempts += 1
            try:
                task.action()
                return TaskResult("success", attempts=attempts, error=None)
            except Exception as exc:  # noqa: BLE001 - retried below, converted to a result
                last_error = str(exc)
                if attempts < task.max_attempts:
                    self._clock.advance(task.retry_backoff)
        return TaskResult("failed", attempts=attempts, error=last_error)

    def _release_dependents(self, tid, remaining, results, ready):
        for other_tid, deps in remaining.items():
            if other_tid in results or tid not in deps:
                continue
            if all(d in results for d in deps):
                ready.push(other_tid)
