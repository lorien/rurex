"""Tests for benchmark measurement guards."""

import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).parents[1] / "benchmarks"))
import measurement  # noqa: E402


def test_efficiency_gil_uses_process_cpu():
    eff, kind = measurement.efficiency([0.4, 0.5], [1.0, 1.0], gil=True)
    assert kind == "process"
    assert 0.89 < eff < 0.91


def test_efficiency_gil_parallel_uses_per_worker():
    eff, kind = measurement.efficiency([1.0, 1.0], [1.0, 1.0], gil=True)
    assert kind == "per-worker"
    assert eff == 1.0


def test_efficiency_free_uses_min_worker():
    eff, kind = measurement.efficiency([1.0, 0.2], [1.0, 1.0], gil=False)
    assert kind == "per-worker"
    assert eff == 0.2


def test_quality_rejects_low_efficiency():
    ok, reason = measurement.quality_ok({"efficiency": 0.5}, 0.9, None)
    assert not ok
    assert "efficiency" in reason


def test_quality_rejects_high_load():
    info = {"efficiency": 1.0, "loadavg_before": [2.0, 0.1, 0.1]}
    ok, reason = measurement.quality_ok(info, 0.9, 1.0)
    assert not ok
    assert "load" in reason


def test_quality_accepts_clean_point():
    info = {"efficiency": 0.99, "loadavg_before": [0.1, 0.1, 0.1]}
    assert measurement.quality_ok(info, 0.9, 1.0) == (True, "")


def test_limits_from_environment(monkeypatch):
    monkeypatch.setenv("RUREX_MIN_EFFICIENCY", "0.75")
    monkeypatch.setenv("RUREX_RETRIES", "4")
    monkeypatch.setenv("RUREX_MAX_LOAD", "1.5")
    assert measurement.min_efficiency() == 0.75
    assert measurement.retries() == 4
    assert measurement.max_load() == 1.5
    monkeypatch.delenv("RUREX_MAX_LOAD")
    assert measurement.max_load() is None
