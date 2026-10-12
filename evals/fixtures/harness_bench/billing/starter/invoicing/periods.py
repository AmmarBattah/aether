import calendar
from datetime import date

from .errors import InvalidInput
from .money import to_decimal


def validate_date(value, name):
    if not isinstance(value, date):
        raise InvalidInput(f"{name} must be a datetime.date")
    return value


def next_anchor(current, anchor_day):
    validate_date(current, "current")
    if not isinstance(anchor_day, int) or not 1 <= anchor_day <= 31:
        raise InvalidInput("anchor_day must be an int from 1 to 31")
    year, month = (current.year + 1, 1) if current.month == 12 else (current.year, current.month + 1)
    return date(year, month, min(anchor_day, calendar.monthrange(year, month)[1]))


def validate_window(period_start, period_end, active_from, active_to):
    validate_date(period_start, "period_start")
    validate_date(period_end, "period_end")
    if period_end <= period_start:
        raise InvalidInput("period_end must be after period_start")


def prorate(amount, period_start, period_end, active_from=None, active_to=None):
    amount = to_decimal(amount)
    validate_window(period_start, period_end, active_from, active_to)
    start = period_start if active_from is None else active_from
    end = period_end if active_to is None else min(period_end, active_to)
    overlap = (end - start).days
    if overlap <= 0:
        return 0
    return amount * overlap / (period_end - period_start).days
