import calendar
from datetime import date, datetime
from decimal import Decimal

from .errors import InvalidInput
from .money import to_decimal


def validate_date(value, name):
    if not isinstance(value, date) or isinstance(value, datetime):
        raise InvalidInput(f"{name} must be a datetime.date")
    return value


def _optional_date(value, name):
    return None if value is None else validate_date(value, name)


def _billing_date(year, month, anchor_day):
    return date(year, month, min(anchor_day, calendar.monthrange(year, month)[1]))


def next_anchor(current, anchor_day):
    validate_date(current, "current")
    if isinstance(anchor_day, bool) or not isinstance(anchor_day, int) or not 1 <= anchor_day <= 31:
        raise InvalidInput("anchor_day must be an int from 1 to 31")
    candidate = _billing_date(current.year, current.month, anchor_day)
    if candidate > current:
        return candidate
    year, month = (current.year + 1, 1) if current.month == 12 else (current.year, current.month + 1)
    return _billing_date(year, month, anchor_day)


def validate_window(period_start, period_end, active_from, active_to):
    validate_date(period_start, "period_start")
    validate_date(period_end, "period_end")
    if period_end <= period_start:
        raise InvalidInput("period_end must be after period_start")
    active_from = _optional_date(active_from, "active_from")
    active_to = _optional_date(active_to, "active_to")
    if active_from is not None and active_to is not None and active_to < active_from:
        raise InvalidInput("active_to must not be before active_from")


def prorate(amount, period_start, period_end, active_from=None, active_to=None):
    amount = to_decimal(amount)
    validate_window(period_start, period_end, active_from, active_to)
    start = period_start if active_from is None else max(period_start, active_from)
    end = period_end if active_to is None else min(period_end, active_to)
    overlap = (end - start).days
    if overlap <= 0:
        return Decimal(0)
    return amount * overlap / (period_end - period_start).days
