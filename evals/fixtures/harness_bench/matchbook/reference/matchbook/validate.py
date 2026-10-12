"""Argument validation shared by live calls and journal replay."""
from .errors import Invalid

SIDES = ("buy", "sell")
TIFS = ("GTC", "IOC", "FOK", "GTD")
STPS = ("cancel_newest", "cancel_oldest", "decrement")
PARAMETERS = {
    "submit": ("order_id", "owner", "side", "price", "quantity", "now", "tif", "expires", "post_only", "stp"),
    "submit_iceberg": ("order_id", "owner", "side", "price", "quantity", "display", "now", "stp"),
    "amend": ("order_id", "owner", "now", "price", "quantity"),
    "cancel": ("order_id", "owner", "now"),
    "advance": ("now",),
}


def _is_int(value):
    return type(value) is int


def _require(condition, message):
    if not condition:
        raise Invalid(message)


def _name(value, label):
    _require(type(value) is str and value != "", f"{label} must be a nonempty string")


def check_now(now, clock):
    _require(_is_int(now) and now >= 0, "now must be a nonnegative integer")
    _require(now >= clock, "time cannot go backwards")


def check_submit(args, clock):
    _name(args["order_id"], "order_id")
    _name(args["owner"], "owner")
    _require(args["side"] in SIDES and type(args["side"]) is str, "side must be buy or sell")
    price, tif = args["price"], args["tif"]
    _require(price is None or (_is_int(price) and price > 0), "price must be a positive integer or None")
    _require(_is_int(args["quantity"]) and args["quantity"] > 0, "quantity must be a positive integer")
    check_now(args["now"], clock)
    _require(type(tif) is str and tif in TIFS, "unknown tif")
    _require(type(args["stp"]) is str and args["stp"] in STPS, "unknown stp")
    _require(type(args["post_only"]) is bool, "post_only must be a bool")
    if tif == "GTD":
        _require(_is_int(args["expires"]) and args["expires"] > args["now"], "GTD needs expires after now")
    else:
        _require(args["expires"] is None, "expires is only valid for GTD")
    if price is None:
        _require(tif in ("IOC", "FOK"), "a market order must be IOC or FOK")
    if args["post_only"]:
        _require(price is not None and tif in ("GTC", "GTD"), "post_only needs a resting limit order")


def check_submit_iceberg(args, clock):
    _name(args["order_id"], "order_id")
    _name(args["owner"], "owner")
    _require(type(args["side"]) is str and args["side"] in SIDES, "side must be buy or sell")
    _require(_is_int(args["price"]) and args["price"] > 0, "an iceberg needs a positive integer price")
    _require(_is_int(args["quantity"]) and args["quantity"] > 0, "quantity must be a positive integer")
    _require(_is_int(args["display"]) and 0 < args["display"] <= args["quantity"],
             "display must be a positive integer not above quantity")
    check_now(args["now"], clock)
    _require(type(args["stp"]) is str and args["stp"] in STPS, "unknown stp")


def check_amend(args, clock):
    _name(args["order_id"], "order_id")
    _name(args["owner"], "owner")
    check_now(args["now"], clock)
    price, quantity = args["price"], args["quantity"]
    _require(price is None or (_is_int(price) and price > 0), "price must be a positive integer or None")
    _require(quantity is None or (_is_int(quantity) and quantity > 0), "quantity must be a positive integer or None")
    _require(price is not None or quantity is not None, "amend needs a price or a quantity")


def check_cancel(args, clock):
    _name(args["order_id"], "order_id")
    _name(args["owner"], "owner")
    check_now(args["now"], clock)


def check_advance(args, clock):
    check_now(args["now"], clock)
