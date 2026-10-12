from stats import mean, variance


def test_mean():
    assert mean([2, 4, 6]) == 4


def test_variance():
    assert variance([2, 4, 4, 4, 5, 5, 7, 9]) == 4
