import unittest
from smart_cabinet.models import EventType, LedState, SourceMode, TelemetryEvent, led_for_known_state

class SmartCabinetTests(unittest.TestCase):
    def test_ont_event_contains_required_field_evidence(self):
        event = TelemetryEvent(cabinet_id="CAB-A", event_type=EventType.ONT_PRESENCE, source_mode=SourceMode.SIMULATED, ont_present=True, gpon_serial="ONT-DEMO", splitter_id="SPL-A-01", port_id="PORT-A-01", optical_rx_dbm=-19.4)
        self.assertEqual(event.as_record()["source_mode"], "SIMULATED")

    def test_port_requires_splitter_context(self):
        event = TelemetryEvent(cabinet_id="CAB-A", event_type=EventType.PORT_STATE_CHANGED, source_mode=SourceMode.SIMULATED, port_id="PORT-A-01")
        with self.assertRaises(ValueError): event.validate()

    def test_led_is_output_not_telemetry_source(self):
        led = led_for_known_state("RESERVED")
        self.assertEqual(led.state, LedState.AMBER)
        self.assertNotIn("sensor", led.__dict__)

if __name__ == "__main__": unittest.main()

