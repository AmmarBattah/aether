import unittest
from eventlog import Hub
class SmokeTests(unittest.TestCase):
    def test_publish_and_poll(self):
        records=[]; hub=Hub(records.append); hub.publish("one", {"x":1}); hub.create_group("g")
        c=hub.claim("g","worker",1,4)
        self.assertEqual([e["key"] for e in hub.poll("g","worker",c["generation"],5,1)],["one"])
    def test_cursor_survives_replay(self):
        records=[]; hub=Hub(records.append); hub.publish("one",1); hub.create_group("g")
        c=hub.claim("g","w",0,5); hub.ack("g","w",c["generation"],1,1)
        recovered=Hub(lambda r:None,records)
        self.assertEqual(recovered.get_group("g")["committed"],1)
