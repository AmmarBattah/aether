"""Parallel blocks: pairing, counterbalance, isolation and owned-process cleanup. No model calls."""
import json
import os
import shutil
import sys
import tempfile
import threading
import time
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).parent))

import harness_bench as hb
import harness_bench_blocks as blocks
import harness_bench_process as process
from harness_bench_test_support import TEST_SCRATCH

PAIRS = ["native-haiku-low", "aether-single-haiku-low", "native-luna-low", "aether-single-luna-low"]
CASES = ["matchbook", "gridcalc", "ledger"]


def schedule(jobs, runs=2, seed=0, conditions=PAIRS, cases=CASES):
    return blocks.paired_blocks(hb.trial_order(cases, conditions, runs, seed), jobs, seed)


class PairedBlockTests(unittest.TestCase):
    def test_pair_label_matches_native_with_its_aether_twin_only(self):
        self.assertEqual(blocks.pair_label("native-haiku-low"), blocks.pair_label("aether-single-haiku-low"))
        self.assertEqual(blocks.pair_label("native-claude"), blocks.pair_label("aether-single-claude"))
        self.assertNotEqual(blocks.pair_label("native-haiku-low"), blocks.pair_label("native-luna-low"))
        self.assertNotEqual(blocks.pair_label("native-luna-low"), blocks.pair_label("aether-solo-minimal-luna-low"))
        self.assertEqual(blocks.pair_label("aether-coordinator-team"), "aether-coordinator-team")

    def test_one_job_keeps_the_sequential_order(self):
        order = hb.trial_order(CASES, PAIRS, 2, 7)
        self.assertEqual(blocks.paired_blocks(order, 1, 7), [[trial] for trial in order])

    def test_every_trial_runs_exactly_once_and_no_block_exceeds_jobs(self):
        for jobs in (2, 3, 4, 8):
            planned = schedule(jobs)
            flat = [trial for block in planned for trial in block]
            self.assertEqual(sorted(flat), sorted(hb.trial_order(CASES, PAIRS, 2)), jobs)
            self.assertEqual(len(set(flat)), len(CASES) * len(PAIRS) * 2, jobs)
            self.assertTrue(all(len(block) <= jobs for block in planned), jobs)

    def test_a_matched_pair_always_shares_one_block(self):
        for jobs in (2, 3, 4, 8):
            for seed in (0, 5):
                for index, block in enumerate(schedule(jobs, seed=seed)):
                    in_block = {(case, blocks.pair_label(key), run): 0 for case, key, run in block}
                    for case, key, run in block:
                        in_block[(case, blocks.pair_label(key), run)] += 1
                    self.assertEqual(set(in_block.values()), {2}, (jobs, seed, index, block))

    def test_two_jobs_run_one_pair_at_a_time(self):
        planned = schedule(2, runs=1)
        self.assertEqual(len(planned), len(CASES) * 2)
        for block in planned:
            self.assertEqual({key.startswith("native-") for _, key, _ in block}, {True, False})
            self.assertEqual(len({(case, run) for case, _, run in block}), 1)

    def test_which_member_launches_first_is_counterbalanced(self):
        planned = schedule(4, runs=4)
        first = {}
        for block in planned:
            seen = set()
            for case, key, run in block:
                pair = (case, blocks.pair_label(key), run)
                if pair not in seen:
                    seen.add(pair)
                    first.setdefault(blocks.pair_label(key), []).append(key.startswith("native-"))
        for label, leads in first.items():
            self.assertEqual(len(leads), len(CASES) * 4, label)
            self.assertEqual(sum(leads) * 2, len(leads), f"{label}: native led {sum(leads)} of {len(leads)}")

    def test_the_seed_flips_the_first_member_and_replays(self):
        even, odd = schedule(2, runs=1, seed=0, cases=["matchbook"], conditions=PAIRS[:2]), None
        odd = blocks.paired_blocks(hb.trial_order(["matchbook"], PAIRS[:2], 1, 0), 2, seed=1)
        self.assertEqual(even[0][0][1], "native-haiku-low")
        self.assertEqual(odd[0][0][1], "aether-single-haiku-low")
        self.assertEqual(schedule(4, seed=11), schedule(4, seed=11))

    def test_unpaired_cells_fill_blocks_on_their_own(self):
        planned = schedule(3, runs=1, cases=["matchbook"],
                           conditions=["native-luna-low", "aether-solo-minimal-luna-low", "native-claude"])
        self.assertEqual([len(block) for block in planned], [3])

    def test_jobs_outside_the_bound_are_refused(self):
        order = hb.trial_order(CASES, PAIRS, 1)
        for jobs in (0, -1, blocks.MAX_JOBS + 1):
            with self.assertRaises(AssertionError):
                blocks.paired_blocks(order, jobs)
            with self.assertRaises(SystemExit), patch("sys.stderr"):
                hb.parse_args(["--jobs", str(jobs)])
        self.assertEqual(hb.parse_args([]).jobs, 1)

    def test_load_average_is_three_numbers_or_unknown(self):
        load = blocks.host_load_average()
        self.assertEqual(len(load), 3)
        self.assertTrue(all(isinstance(value, float) and value >= 0 for value in load))
        with patch.object(blocks.os, "getloadavg", side_effect=OSError("unsupported")):
            self.assertIsNone(blocks.host_load_average())


class RunBlockTests(unittest.TestCase):
    def test_trials_of_a_block_really_overlap(self):
        together = threading.Barrier(4)

        def run_one(trial, position):
            together.wait(timeout=10)
            return {"trial": trial, "position": position}
        rows = list(blocks.run_block(["a", "b", "c", "d"], run_one))
        self.assertEqual(sorted(row["trial"] for row in rows), ["a", "b", "c", "d"])
        self.assertEqual(sorted(row["position"] for row in rows), [1, 2, 3, 4])

    def test_a_crashing_trial_stops_every_owned_group_and_propagates(self):
        def run_one(trial, position):
            if trial == "bad":
                raise KeyboardInterrupt("operator stop")
            return {"trial": trial}
        with patch.object(blocks, "terminate_owned_process_groups") as terminate:
            with self.assertRaises(KeyboardInterrupt):
                list(blocks.run_block(["ok", "bad"], run_one))
        terminate.assert_called_once_with()


class OwnedProcessTests(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(dir=TEST_SCRATCH, prefix="hb-blocks-"))
        self.addCleanup(shutil.rmtree, self.tmp, ignore_errors=True)
        self.addCleanup(setattr, process, "_SHUTTING_DOWN", False)

    def _gone(self, pid):
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            stat = Path(f"/proc/{pid}/stat")
            if not stat.exists() or stat.read_text().rsplit(")", 1)[1].split()[0] == "Z":
                return True
            time.sleep(0.05)
        return False

    def _run(self, name, script, timeout, results):
        results[name] = hb.run_subprocess_capturing(
            ["sh", "-c", script], str(self.tmp), timeout, str(self.tmp / f"{name}.out"), str(self.tmp / f"{name}.err"))

    def test_a_timeout_stops_its_own_tree_and_no_other_trial(self):
        results = {}
        slow = f"sleep 60 & echo $! > {self.tmp}/slow.pid; wait"
        steady = f"sleep 1.5 & echo $! > {self.tmp}/steady.pid; wait; echo finished"
        threads = [threading.Thread(target=self._run, args=("slow", slow, 0.5, results)),
                   threading.Thread(target=self._run, args=("steady", steady, 30, results))]
        for thread in threads:
            thread.start()
        for thread in threads:
            thread.join(timeout=40)
        self.assertTrue(results["slow"]["timed_out"])
        self.assertIsNone(results["slow"]["cleanup_error"])
        self.assertTrue(self._gone(int((self.tmp / "slow.pid").read_text())), "timed-out trial left a child")
        self.assertFalse(results["steady"]["timed_out"])
        self.assertEqual(results["steady"]["exit_code"], 0)
        self.assertEqual((self.tmp / "steady.out").read_text().strip(), "finished")
        self.assertEqual(process._OWNED, set(), "finished trials must not stay registered")

    def test_shutdown_stops_owned_groups_and_refuses_new_ones(self):
        results = {}
        script = f"sleep 60 & echo $! > {self.tmp}/owned.pid; wait"
        thread = threading.Thread(target=self._run, args=("owned", script, 30, results))
        thread.start()
        deadline = time.monotonic() + 5
        while not (self.tmp / "owned.pid").exists() and time.monotonic() < deadline:
            time.sleep(0.02)
        self.assertEqual(process.terminate_owned_process_groups(), 1)
        thread.join(timeout=20)
        self.assertFalse(thread.is_alive())
        self.assertFalse(results["owned"]["timed_out"])
        self.assertNotEqual(results["owned"]["exit_code"], 0)
        self.assertTrue(self._gone(int((self.tmp / "owned.pid").read_text())))
        with self.assertRaisesRegex(RuntimeError, "shutting down"):
            self._run("late", "sleep 60", 30, results)
        self.assertNotIn("late", results)


class TrialIsolationTests(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(dir=TEST_SCRATCH, prefix="hb-iso-"))
        self.addCleanup(shutil.rmtree, self.tmp, ignore_errors=True)
        self.runtime = Path(tempfile.mkdtemp(prefix="hbt-"))
        self.addCleanup(shutil.rmtree, self.runtime, ignore_errors=True)

    def test_concurrent_trials_get_disjoint_stores_and_no_live_routing(self):
        seen, lock = [], threading.Lock()

        def refuse_start(_binary, env, _workspace):
            with lock:
                seen.append(dict(env))
            raise RuntimeError("no daemon in this test")
        live = {"AETHER_SOCKET": "/operator/live.sock", "AETHER_ADDRESS": "live:123", "AETHER_TOKEN": "secret",
                "AETHER_CONFIG_DIR": "/operator/config", "AETHER_DATA_DIR": "/operator/data",
                "XDG_RUNTIME_DIR": str(self.runtime)}

        def run_one(trial, _position):
            trial_dir = self.tmp / trial
            trial_dir.mkdir()
            result = {}
            hb.run_aether_trial("/fake", hb.CONDITIONS["aether-single-haiku-low"], "task", trial_dir,
                                1, str(trial_dir), "/bin/true", result)
            return result
        with patch.dict(os.environ, live), patch.object(hb, "start_daemon", side_effect=refuse_start), \
                patch.object(hb, "stop_daemon", return_value=None):
            results = list(blocks.run_block(["t1", "t2", "t3", "t4"], run_one))
        self.assertEqual(len(seen), 4)
        for key in ("AETHER_SOCKET", "AETHER_CONFIG_DIR", "AETHER_DATA_DIR"):
            self.assertEqual(len({env[key] for env in seen}), 4, f"{key} is shared between trials")
            self.assertTrue(all(not env[key].startswith("/operator") for env in seen), key)
        for env in seen:
            self.assertEqual(sorted(k for k in env if k.startswith("AETHER_")),
                             ["AETHER_CONFIG_DIR", "AETHER_DATA_DIR", "AETHER_SOCKET"])
            self.assertTrue(Path(env["AETHER_DATA_DIR"]).resolve().is_relative_to(self.tmp.resolve()))
            self.assertLess(len(env["AETHER_SOCKET"].encode()), hb.SOCKET_PATH_LIMIT)
        for result in results:
            self.assertIn("AETHER_ADDRESS", result["env_isolation"]["stripped_vars"])
            self.assertIn("AETHER_TOKEN", result["env_isolation"]["stripped_vars"])
            self.assertIsNone(result["daemon_cleanup_error"])
        self.assertEqual(list(self.runtime.iterdir()), [], "private socket directories must be removed")

    def _deep_trial(self, stop_error=None, proxy_error=None, removal_error=None):
        """One trial whose path is too long for a socket, so it owns a runtime directory."""
        deep = self.tmp / ("d" * 90)
        deep.mkdir()
        result, made = {}, []
        real_remove = hb.remove_socket_dir

        def remove(socket_dir):
            made.append(socket_dir)
            return removal_error or real_remove(socket_dir)
        proxy = patch.object(hb, "install_claude_receipt_proxy", side_effect=proxy_error) if proxy_error \
            else patch.object(hb, "start_daemon", side_effect=RuntimeError("no daemon in this test"))
        with patch.dict(os.environ, {"XDG_RUNTIME_DIR": str(self.runtime)}), proxy, \
                patch.object(hb, "stop_daemon", return_value=stop_error), \
                patch.object(hb, "remove_socket_dir", side_effect=remove):
            try:
                hb.run_aether_trial("/fake", hb.CONDITIONS["aether-single-haiku-low"], "task", deep,
                                    1, str(deep), "/bin/true", result)
            except OSError as error:
                result["raised"] = error
        self.assertEqual(len(made), 1, "the private socket directory is removed exactly once")
        self.assertTrue(result["env_isolation"]["socket_in_runtime_dir"])
        return result

    def test_socket_directory_is_removed_when_stopping_the_daemon_reports_an_error(self):
        result = self._deep_trial(stop_error="daemon stop exited 1: refused")
        self.assertEqual(result["daemon_cleanup_error"], "daemon stop exited 1: refused")
        self.assertEqual(list(self.runtime.iterdir()), [], "a stop error must not leak the socket directory")

    def test_socket_directory_is_removed_when_setup_raises_before_the_daemon_starts(self):
        result = self._deep_trial(proxy_error=OSError("proxy could not be installed"))
        self.assertEqual(str(result["raised"]), "proxy could not be installed")
        self.assertIsNone(result["daemon_cleanup_error"])
        self.assertEqual(list(self.runtime.iterdir()), [], "a setup failure must not leak the socket directory")

    def test_stop_and_removal_errors_are_both_recorded(self):
        result = self._deep_trial(stop_error="daemon socket remains after shutdown",
                                  removal_error="PermissionError: denied")
        self.assertEqual(result["daemon_cleanup_error"],
                         "daemon socket remains after shutdown; PermissionError: denied")
        self.assertEqual(len(list(self.runtime.iterdir())), 1, "the failed removal is what the error reports")

    def test_long_trial_path_moves_only_the_socket_to_a_private_directory(self):
        deep = self.tmp / ("d" * 90)
        deep.mkdir()
        with patch.dict(os.environ, {"XDG_RUNTIME_DIR": str(self.runtime)}):
            first, first_dir = hb.isolated_socket_path(str(deep))
            second, second_dir = hb.isolated_socket_path(str(deep))
            short, short_dir = hb.isolated_socket_path("/x")
        self.assertNotEqual(first_dir, second_dir)
        self.assertEqual(Path(first).parent, Path(first_dir))
        self.assertEqual(os.stat(first_dir).st_mode & 0o777, 0o700)
        self.assertEqual((short, short_dir), ("/x/aetherd.sock", None))
        self.assertIsNone(hb.remove_socket_dir(first_dir))
        self.assertIsNone(hb.remove_socket_dir(None))
        self.assertIn("FileNotFoundError", hb.remove_socket_dir(first_dir))
        with patch.dict(os.environ, {"XDG_RUNTIME_DIR": str(self.tmp / "missing")}):
            with self.assertRaisesRegex(ValueError, "XDG_RUNTIME_DIR"):
                hb.isolated_socket_path(str(deep))


class ManifestConcurrencyTests(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(dir=TEST_SCRATCH, prefix="hb-main-"))
        self.addCleanup(shutil.rmtree, self.tmp, ignore_errors=True)

    def fake_trial(self, _binary, condition_key, case, run_index, *_args):
        trial_dir = self.tmp / f"{condition_key}-{case['id']}-{run_index}"
        trial_dir.mkdir()
        return {"condition": condition_key, "case": case["id"], "run": run_index, "trial_dir": str(trial_dir),
                "short_id": trial_dir.name, "status": "completed", "success": True}

    def run_main(self, *extra):
        argv = ["--conditions", "native-haiku-low,aether-single-haiku-low", "--cases", "ledger,scheduler",
                "--runs", "1", "--output-dir", str(self.tmp / "out"), "--binary", "/fake", *extra]
        with patch.object(hb, "pin_binary", return_value=("/fake", "a" * 64)), \
                patch.object(hb, "fetch_price_table", return_value={"models": []}), \
                patch.object(hb, "run_trial", side_effect=self.fake_trial), \
                patch.object(hb, "rebuild_trial_usage"), patch("builtins.print"):
            return json.loads(Path(hb.main(argv)).read_text())

    def test_manifest_records_jobs_blocks_and_load_for_every_trial(self):
        manifest = self.run_main("--jobs", "2", "--pilot")
        self.assertEqual((manifest["jobs"], manifest["blocks"], manifest["pilot"]), (2, blocks.BLOCK_SCHEME, True))
        self.assertEqual([row["sequence"] for row in manifest["trials"]], [1, 2, 3, 4])
        for row in manifest["trials"]:
            self.assertEqual(row["concurrency"]["jobs"], 2)
            self.assertEqual(row["concurrency"]["block_trials"], 2)
            self.assertIn(row["concurrency"]["launch_position"], (1, 2))
            for moment in ("start", "end"):
                self.assertEqual(len(row["host_load_average"][moment]), 3)
            saved = json.loads(Path(row["trial_dir"], "result.json").read_text())
            self.assertEqual(saved["concurrency"], row["concurrency"])
        by_block = {}
        for row in manifest["trials"]:
            by_block.setdefault(row["concurrency"]["block"], set()).add((row["case"], row["condition"]))
        self.assertEqual(sorted(len(cells) for cells in by_block.values()), [2, 2])
        for cells in by_block.values():
            self.assertEqual(len({case for case, _ in cells}), 1, "a block must hold one case's matched pair")

    def test_default_run_is_sequential_and_not_a_pilot(self):
        manifest = self.run_main()
        self.assertEqual((manifest["jobs"], manifest["pilot"]), (1, False))
        self.assertEqual([row["concurrency"]["block_trials"] for row in manifest["trials"]], [1, 1, 1, 1])
        self.assertEqual([row["concurrency"]["block"] for row in manifest["trials"]], [1, 2, 3, 4])


if __name__ == "__main__":
    unittest.main()
