import copy
import unittest
from eventlog import Busy, Conflict, Hub, Invalid, StaleClaim

class Journal:
    def __init__(self): self.rows=[]; self.fail=None; self.mutate=False
    def append(self, row):
        if self.fail: raise self.fail
        self.rows.append(row)
        if self.mutate:
            row.clear()

def setup(events=0, start="earliest"):
    j=Journal(); h=Hub(j.append)
    for i in range(events): h.publish(f"e{i}", {"n": i})
    h.create_group("g", start)
    return h,j

def claim(h, now=0, ttl=5): return h.claim("g", "w", now, ttl)

class EventLogTests(unittest.TestCase):
    def test_publish_offset_starts_at_one(self): self.assertEqual(setup()[0].publish("a",1)["offset"],1)
    def test_offsets_are_contiguous(self):
        h,_=setup(); self.assertEqual([h.publish(str(i),i)["offset"] for i in range(4)],[1,2,3,4])
    def test_publish_duplicate_is_idempotent_and_journaled(self):
        h,j=setup(); a=h.publish("x",[1]); b=h.publish("x",[1]); self.assertEqual(a,b); self.assertEqual(len(j.rows),3)
    def test_publish_duplicate_payload_conflict(self):
        h,_=setup(); h.publish("x",1)
        with self.assertRaises(Conflict): h.publish("x",2)
    def test_payload_equality_is_type_sensitive(self):
        h,_=setup(); h.publish("x",True)
        with self.assertRaises(Conflict): h.publish("x",1)
    def test_nested_payload_equality_is_type_sensitive(self):
        h,_=setup(); h.publish("x",{"a":[True]})
        with self.assertRaises(Conflict): h.publish("x",{"a":[1]})
    def test_publish_rejects_empty_key(self):
        with self.assertRaises(Invalid): setup()[0].publish("",1)
    def test_publish_rejects_nonstring_key(self):
        with self.assertRaises(Invalid): setup()[0].publish(3,1)
    def test_publish_rejects_nan(self):
        with self.assertRaises(Invalid): setup()[0].publish("x",float("nan"))
    def test_publish_rejects_infinity(self):
        with self.assertRaises(Invalid): setup()[0].publish("x",float("inf"))
    def test_publish_rejects_non_json_value(self):
        with self.assertRaises(Invalid): setup()[0].publish("x",object())
    def test_publish_rejects_nonstring_object_keys(self):
        with self.assertRaises(Invalid): setup()[0].publish("x",{1:"a"})
    def test_publish_rejects_cycles_atomically(self):
        x=[]; x.append(x); h,j=setup()
        with self.assertRaises(Invalid): h.publish("x",x)
        self.assertEqual((h.snapshot()["sequence"],len(j.rows)),(1,1))
    def test_create_group_earliest_starts_at_zero(self): self.assertEqual(setup(3)[0].get_group("g")["committed"],0)
    def test_create_group_latest_starts_at_tail(self): self.assertEqual(setup(3,"latest")[0].get_group("g")["committed"],3)
    def test_same_group_creation_is_journaled_idempotency(self):
        h,j=setup(); h.create_group("g"); self.assertEqual(len(j.rows),2)
    def test_group_start_conflict(self):
        h,_=setup()
        with self.assertRaises(Conflict): h.create_group("g","latest")
    def test_invalid_group_start(self):
        with self.assertRaises(Invalid): setup()[0].create_group("bad","middle")
    def test_claim_initial_generation_is_one(self): self.assertEqual(claim(setup()[0])["generation"],1)
    def test_same_owner_renewal_keeps_generation(self):
        h,_=setup(); a=claim(h); b=h.claim("g","w",2,8); self.assertEqual((a["generation"],b["generation"]),(1,1))
    def test_other_owner_is_blocked_before_deadline(self):
        h,_=setup(); claim(h)
        with self.assertRaises(Busy): h.claim("g","other",4,1)
    def test_expiry_equality_allows_new_owner(self):
        h,_=setup(); claim(h,1,4); self.assertEqual(h.claim("g","other",5,2)["generation"],2)
    def test_expired_owner_cannot_ack(self):
        h,_=setup(1); c=claim(h,0,2); h.claim("g","other",2,2)
        with self.assertRaises(StaleClaim): h.ack("g","w",c["generation"],1,2)
    def test_expired_owner_cannot_release(self):
        h,_=setup(); c=claim(h,0,2); h.claim("g","other",2,2)
        with self.assertRaises(StaleClaim): h.release("g","w",c["generation"],2)
    def test_claim_rejects_bool_time(self):
        with self.assertRaises(Invalid): setup()[0].claim("g","w",True,2)
    def test_claim_rejects_zero_ttl(self):
        with self.assertRaises(Invalid): setup()[0].claim("g","w",0,0)
    def test_claim_time_cannot_go_backwards(self):
        h,_=setup(); claim(h,4)
        with self.assertRaises(Invalid): h.claim("g","w",3,2)
    def test_poll_returns_ordered_events(self):
        h,_=setup(3); c=claim(h); self.assertEqual([e["offset"] for e in h.poll("g","w",c["generation"],3,0)],[1,2,3])
    def test_poll_obeys_limit(self):
        h,_=setup(4); c=claim(h); self.assertEqual(len(h.poll("g","w",c["generation"],2,0)),2)
    def test_poll_repeats_until_ack(self):
        h,_=setup(2); c=claim(h); self.assertEqual(h.poll("g","w",c["generation"],2,0),h.poll("g","w",c["generation"],2,0))
    def test_poll_is_read_only(self):
        h,j=setup(2); c=claim(h); before=h.snapshot(); rows=len(j.rows); h.poll("g","w",c["generation"],2,0)
        self.assertEqual((h.snapshot(),len(j.rows)),(before,rows))
    def test_poll_is_defensive(self):
        h,_=setup(1); c=claim(h); out=h.poll("g","w",c["generation"],1,0); out[0]["payload"]["n"]=9
        self.assertEqual(h.poll("g","w",c["generation"],1,0)[0]["payload"]["n"],0)
    def test_poll_rejects_zero_limit(self):
        h,_=setup(); c=claim(h)
        with self.assertRaises(Invalid): h.poll("g","w",c["generation"],0,0)
    def test_poll_rejects_stale_generation(self):
        h,_=setup(); claim(h)
        with self.assertRaises(StaleClaim): h.poll("g","w",0,1,0)
    def test_ack_rejects_cursor_beyond_tail(self):
        h,_=setup(); c=claim(h)
        with self.assertRaises(Invalid): h.ack("g","w",c["generation"],1,0)
    def test_ack_rejects_backwards_cursor(self):
        h,_=setup(2); c=claim(h); h.ack("g","w",c["generation"],2,0)
        with self.assertRaises(Invalid): h.ack("g","w",c["generation"],1,0)
    def test_ack_equal_cursor_is_durable_noop(self):
        h,j=setup(); c=claim(h); before=len(j.rows); h.ack("g","w",c["generation"],0,0)
        self.assertEqual(len(j.rows),before+1)
    def test_ack_rejects_bool_cursor(self):
        h,_=setup(); c=claim(h)
        with self.assertRaises(Invalid): h.ack("g","w",c["generation"],False,0)
    def test_release_clears_owner_but_keeps_fence(self):
        h,_=setup(); c=claim(h); g=h.release("g","w",c["generation"],0)
        self.assertEqual((g["owner"],g["lease_until"],g["generation"]),(None,None,1))
    def test_release_is_fenced(self):
        h,_=setup(); claim(h)
        with self.assertRaises(StaleClaim): h.release("g","w",99,0)
    def test_failed_append_leaves_state_and_sequence_unchanged(self):
        h,j=setup(); before=h.snapshot(); exc=OSError("disk"); j.fail=exc
        with self.assertRaises(OSError) as caught: h.publish("x",1)
        self.assertIs(caught.exception,exc); self.assertEqual(h.snapshot(),before)
    def test_append_callback_cannot_mutate_internal_record(self):
        h,j=setup(); j.mutate=True; result=h.publish("x",{"a":1})
        self.assertEqual(result,{"offset":1,"key":"x","payload":{"a":1}})
    def test_returned_group_is_defensive(self):
        h,_=setup(); result=claim(h); result["owner"]="corrupt"
        self.assertEqual(h.get_group("g")["owner"],"w")
    def test_snapshot_is_defensive(self):
        h,_=setup(1); snap=h.snapshot(); snap["events"][0]["payload"]["n"]=8
        self.assertEqual(h.snapshot()["events"][0]["payload"]["n"],0)
    def test_replay_restores_all_state_without_append(self):
        h,j=setup(2); c=claim(h); h.ack("g","w",c["generation"],1,0); before=h.snapshot()
        recovered=Hub(lambda _:self.fail("append called during replay"),copy.deepcopy(j.rows))
        self.assertEqual(recovered.snapshot(),before)
    def test_replay_rejects_sequence_gap(self):
        h,j=setup(); bad=copy.deepcopy(j.rows); bad[0]["sequence"]=2
        with self.assertRaises(Invalid): Hub(lambda _:None,bad)
    def test_replay_rejects_extra_record_key(self):
        h,j=setup(); bad=copy.deepcopy(j.rows); bad[0]["extra"]=1
        with self.assertRaises(Invalid): Hub(lambda _:None,bad)
    def test_replay_rejects_unknown_operation(self):
        with self.assertRaises(Invalid): Hub(lambda _:None,[{"sequence":1,"operation":"wat","arguments":{}}])
    def test_replay_rejects_invalid_transition_and_prefix(self):
        rows=[{"sequence":1,"operation":"create_group","arguments":{"name":"g","start":"earliest"}},
              {"sequence":2,"operation":"ack","arguments":{"group":"g","consumer":"x","generation":1,"through":0,"now":0}}]
        with self.assertRaises(Invalid): Hub(lambda _:None,rows)
