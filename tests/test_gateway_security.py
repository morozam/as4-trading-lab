"""The lab security profile must survive fresh setup and existing-node upgrades."""
from pathlib import Path
import importlib.util
import tempfile
import unittest
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
NS = {'p': 'http://holodeck-b2b.org/schemas/2014/10/pmode'}
AES = 'http://www.w3.org/2009/xmlenc11#aes128-gcm'
RSA = 'http://www.w3.org/2009/xmlenc11#rsa-oaep'


class GatewaySecurityTests(unittest.TestCase):
    def test_every_route_encrypts_for_its_receiver_and_compresses_payloads(self):
        paths = list((ROOT / 'gateway/pmodes').glob('*.xml'))
        self.assertEqual(len(paths), 4)
        for path in paths:
            with self.subTest(route=path.name):
                root = ET.parse(path).getroot()
                receiver = root.find('p:Responder/p:SecurityConfiguration', NS)
                encryption = receiver.find('p:Encryption', NS)
                self.assertIsNotNone(encryption, 'Receiver encryption must be configured')
                self.assertEqual(encryption.findtext('p:Algorithm', namespaces=NS), AES)
                self.assertEqual(encryption.findtext('p:KeyTransport/p:Algorithm', namespaces=NS), RSA)
                signing_key = receiver.find('p:Signing/p:KeystoreAlias', NS)
                encryption_key = encryption.find('p:KeystoreAlias', NS)
                self.assertEqual(encryption_key.text, signing_key.text)
                self.assertEqual(encryption_key.attrib, signing_key.attrib)
                self.assertEqual(root.findtext('p:Leg/p:UserMessageFlow/p:PayloadProfile/p:UseAS4Compression', namespaces=NS), 'true')

    def test_existing_node_upgrade_is_idempotent_and_preserves_routes_and_callbacks(self):
        spec = importlib.util.spec_from_file_location('lab_setup', ROOT / 'setup.py')
        setup = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(setup)
        with tempfile.TemporaryDirectory() as tmp:
            gateway = Path(tmp) / 'gateway'
            (gateway / 'repository/pmodes').mkdir(parents=True)
            (gateway / 'conf').mkdir()
            (gateway / 'lib').mkdir()
            path = gateway / 'repository/pmodes/node-a-forward-sender.xml'
            root = ET.parse(ROOT / 'gateway/pmodes/node-a-forward-sender.xml').getroot()
            sec = root.find('p:Responder/p:SecurityConfiguration', NS)
            for element in list(sec.findall('p:Encryption', NS)):
                sec.remove(element)
            root.find('p:Leg/p:Protocol/p:Address', NS).text = 'http://127.0.0.1:29081/colleague'
            root.find('.//p:DefaultDelivery/p:Parameter/p:value', NS).text = 'http://127.0.0.1:8797/integrations/KEEP_PRIVATE_CALLBACK'
            ET.ElementTree(root).write(path, encoding='UTF-8', xml_declaration=True)
            conf = gateway / 'conf/holodeckb2b.xml'
            conf.write_text('<config><phaseOrder type="InFlow"><phase name="Security"/><phase name="ProtocolProcessing"/></phaseOrder><phaseOrder type="InFaultFlow"><phase name="Security"/><phase name="CoreProcessing"/></phaseOrder></config>')
            setup.install_security(gateway)
            after = ET.parse(path).getroot()
            self.assertEqual(after.findtext('p:Leg/p:Protocol/p:Address', namespaces=NS), 'http://127.0.0.1:29081/colleague')
            self.assertEqual(after.findtext('.//p:DefaultDelivery/p:Parameter/p:value', namespaces=NS), 'http://127.0.0.1:8797/integrations/KEEP_PRIVATE_CALLBACK')
            self.assertIsNotNone(after.find('p:Responder/p:SecurityConfiguration/p:Encryption', NS))
            for flow in ET.parse(conf).getroot().findall('phaseOrder'):
                self.assertEqual([p.get('name') for p in flow][:2], ['Security', 'LabSecurityPolicy'])
                self.assertEqual(flow.find('phase[@name="LabSecurityPolicy"]/handler').get('class'), 'lab.holodeck.RequirePayloadEncryption')
            saved = (path.read_bytes(), conf.read_bytes())
            setup.install_security(gateway)
            self.assertEqual(saved, (path.read_bytes(), conf.read_bytes()))


if __name__ == '__main__':
    unittest.main()
