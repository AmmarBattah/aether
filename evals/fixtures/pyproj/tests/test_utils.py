from utils import calc, clamp


def test_calc():
    assert calc([1, 2, 3]) == 6


def test_clamp():
    assert clamp(5, 0, 3) == 3
    assert clamp(-1, 0, 3) == 0
